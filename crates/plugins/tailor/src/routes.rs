use axum::extract::Path;
use axum::Json;
use platform_kernel::error::AppError;
use platform_kernel::routes::{PluginRoutes, RegistrationError, TenantDb};
use serde::Deserialize;
use serde_json::{json, Value};

const STATUSES: &[&str] = &["received", "cutting", "sewing", "finishing", "ready", "cancelled"];
const DELIVERIES: &[&str] = &["pending", "ready", "out", "delivered"];
const METHODS: &[&str] = &["cash", "card", "transfer", "credit"];

#[derive(Debug, Deserialize)]
struct Measurements {
    #[serde(default)]
    chest: String,
    #[serde(default)]
    waist: String,
    #[serde(default)]
    hip: String,
    #[serde(default)]
    shoulder: String,
    #[serde(default)]
    sleeve: String,
    #[serde(default)]
    length: String,
    #[serde(default)]
    neck: String,
    #[serde(default)]
    thigh: String,
    #[serde(default)]
    inseam: String,
    #[serde(default)]
    other: String,
}

#[derive(Debug, Deserialize)]
struct Cloth {
    #[serde(default, rename = "type")]
    kind: String,
    #[serde(default)]
    color: String,
    #[serde(default)]
    quantity: String,
    #[serde(default = "customer_cloth")]
    source: String,
    #[serde(default)]
    notes: String,
}

fn customer_cloth() -> String {
    "customer".into()
}

#[derive(Debug, Deserialize)]
struct PaymentInput {
    amount: i64,
    method: String,
    #[serde(default)]
    note: String,
}

#[derive(Debug, Deserialize)]
struct NewJob {
    customer_name: String,
    #[serde(default)]
    phone: String,
    garment: String,
    #[serde(default)]
    measurements: Measurements,
    #[serde(default)]
    cloth: Cloth,
    #[serde(default)]
    sewing_details: String,
    #[serde(default)]
    notes: String,
    #[serde(default)]
    delivery_date: String,
    total_amount: i64,
    #[serde(default)]
    payments: Vec<PaymentInput>,
}

impl Default for Measurements {
    fn default() -> Self {
        Self {
            chest: String::new(),
            waist: String::new(),
            hip: String::new(),
            shoulder: String::new(),
            sleeve: String::new(),
            length: String::new(),
            neck: String::new(),
            thigh: String::new(),
            inseam: String::new(),
            other: String::new(),
        }
    }
}

impl Default for Cloth {
    fn default() -> Self {
        Self {
            kind: String::new(),
            color: String::new(),
            quantity: String::new(),
            source: customer_cloth(),
            notes: String::new(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct JobUpdate {
    status: String,
    delivery_status: String,
}

fn clean(value: &str, limit: usize) -> String {
    let text = value.trim();
    text.chars().take(limit).collect()
}

fn require_name(value: &str) -> Result<String, AppError> {
    let name = clean(value, 80);
    if name.is_empty() {
        return Err(AppError::BadRequest("Enter the customer name.".into()));
    }
    Ok(name)
}

fn require_garment(value: &str) -> Result<String, AppError> {
    let garment = clean(value, 80);
    if garment.is_empty() {
        return Err(AppError::BadRequest("Enter what is being sewn.".into()));
    }
    Ok(garment)
}

fn delivery_date(value: &str) -> Result<Option<String>, AppError> {
    let text = value.trim();
    if text.is_empty() {
        return Ok(None);
    }
    if text.len() != 10 || !text.chars().enumerate().all(|(index, ch)| match index {
        4 | 7 => ch == '-',
        _ => ch.is_ascii_digit(),
    }) {
        return Err(AppError::BadRequest("Delivery date is a day, such as 2026-12-01.".into()));
    }
    Ok(Some(text.to_string()))
}

fn check_status(status: &str, delivery: &str) -> Result<(), AppError> {
    if !STATUSES.contains(&status) {
        return Err(AppError::BadRequest("Choose a job status.".into()));
    }
    if !DELIVERIES.contains(&delivery) {
        return Err(AppError::BadRequest("Choose a delivery status.".into()));
    }
    Ok(())
}

fn check_cloth_source(source: &str) -> Result<(), AppError> {
    if source != "customer" && source != "shop" {
        return Err(AppError::BadRequest("Cloth is brought by the customer or from the shop.".into()));
    }
    Ok(())
}

fn check_payment(payment: &PaymentInput, remaining: i64) -> Result<(), AppError> {
    if payment.amount < 1 {
        return Err(AppError::BadRequest("Enter a payment above 0.".into()));
    }
    if payment.amount > remaining {
        return Err(AppError::BadRequest("That payment is more than the balance.".into()));
    }
    if !METHODS.contains(&payment.method.as_str()) {
        return Err(AppError::BadRequest("Choose cash, card, transfer, or credit.".into()));
    }
    if payment.note.chars().count() > 80 {
        return Err(AppError::BadRequest("Keep the payment note to 80 characters.".into()));
    }
    Ok(())
}

fn measurements_json(body: &Measurements) -> Value {
    json!({
        "chest": clean(&body.chest, 20),
        "waist": clean(&body.waist, 20),
        "hip": clean(&body.hip, 20),
        "shoulder": clean(&body.shoulder, 20),
        "sleeve": clean(&body.sleeve, 20),
        "length": clean(&body.length, 20),
        "neck": clean(&body.neck, 20),
        "thigh": clean(&body.thigh, 20),
        "inseam": clean(&body.inseam, 20),
        "other": clean(&body.other, 200),
    })
}

async fn paid(tx: &mut platform_kernel::context::TenantTx<'_>, job_id: i32) -> Result<i64, AppError> {
    let total: i64 = sqlx::query_scalar("SELECT COALESCE(SUM(amount), 0)::bigint FROM tailor_payments WHERE job_id = $1")
        .bind(job_id)
        .fetch_one(tx.conn())
        .await?;
    Ok(total)
}

#[derive(sqlx::FromRow)]
struct JobRecord {
    id: i32,
    customer_name: String,
    phone: String,
    garment: String,
    measurements: Value,
    cloth_type: String,
    cloth_color: String,
    cloth_quantity: String,
    cloth_source: String,
    cloth_notes: String,
    sewing_details: String,
    notes: String,
    total_amount: i64,
    status: String,
    delivery_status: String,
    delivery_date: Option<String>,
    created_at: String,
}

async fn job_json(tx: &mut platform_kernel::context::TenantTx<'_>, id: i32) -> Result<Value, AppError> {
    let row: Option<JobRecord> = sqlx::query_as(
        r#"
        SELECT id, customer_name, phone, garment, measurements,
               cloth_type, cloth_color, cloth_quantity, cloth_source, cloth_notes,
               sewing_details, notes, total_amount, status, delivery_status,
               delivery_date::text AS delivery_date, created_at::text AS created_at
        FROM tailor_jobs
        WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(tx.conn())
    .await?;
    let Some(row) = row else {
        return Err(AppError::NotFound("That job is no longer saved.".into()));
    };
    let payments: Vec<(i32, i64, String, String, String)> = sqlx::query_as(
        "SELECT id, amount, method, note, paid_on::text FROM tailor_payments WHERE job_id = $1 ORDER BY id",
    )
    .bind(id)
    .fetch_all(tx.conn())
    .await?;
    let paid_amount: i64 = payments.iter().map(|payment| payment.1).sum();
    Ok(json!({
        "id": row.id,
        "customer_name": row.customer_name,
        "phone": row.phone,
        "garment": row.garment,
        "measurements": row.measurements,
        "cloth": {
            "type": row.cloth_type,
            "color": row.cloth_color,
            "quantity": row.cloth_quantity,
            "source": row.cloth_source,
            "notes": row.cloth_notes,
        },
        "sewing_details": row.sewing_details,
        "notes": row.notes,
        "total_amount": row.total_amount,
        "paid_amount": paid_amount,
        "balance": row.total_amount - paid_amount,
        "status": row.status,
        "delivery_status": row.delivery_status,
        "delivery_date": row.delivery_date,
        "created_at": row.created_at,
        "payments": payments.into_iter().map(|(id, amount, method, note, paid_on)| json!({
            "id": id, "amount": amount, "method": method, "note": note, "paid_on": paid_on
        })).collect::<Vec<_>>(),
    }))
}

async fn list(db: TenantDb) -> Result<Json<Value>, AppError> {
    let mut tx = db.begin(true).await?;
    let rows: Vec<(i32, String, String, Option<String>, String, String, i64, i64)> = sqlx::query_as(
        r#"
        SELECT j.id, j.customer_name, j.garment, j.delivery_date::text, j.status, j.delivery_status,
               j.total_amount, COALESCE(SUM(p.amount), 0)::bigint
        FROM tailor_jobs j
        LEFT JOIN tailor_payments p ON p.job_id = j.id
        GROUP BY j.id
        ORDER BY j.created_at DESC, j.id DESC
        "#,
    )
    .fetch_all(tx.conn())
    .await?;
    tx.commit().await?;
    Ok(Json(json!({
        "jobs": rows.into_iter().map(|(id, customer_name, garment, delivery_date, status, delivery_status, total_amount, paid_amount)| json!({
            "id": id,
            "customer_name": customer_name,
            "garment": garment,
            "delivery_date": delivery_date,
            "status": status,
            "delivery_status": delivery_status,
            "total_amount": total_amount,
            "paid_amount": paid_amount,
            "balance": total_amount - paid_amount,
        })).collect::<Vec<_>>()
    })))
}

async fn get_job(db: TenantDb, Path(id): Path<i32>) -> Result<Json<Value>, AppError> {
    let mut tx = db.begin(true).await?;
    let job = job_json(&mut tx, id).await?;
    tx.commit().await?;
    Ok(Json(job))
}

async fn create(db: TenantDb, Json(body): Json<NewJob>) -> Result<Json<Value>, AppError> {
    let customer_name = require_name(&body.customer_name)?;
    let garment = require_garment(&body.garment)?;
    let delivery = delivery_date(&body.delivery_date)?;
    check_cloth_source(&body.cloth.source)?;
    if body.total_amount < 0 || body.total_amount > 100_000_000 {
        return Err(AppError::BadRequest("Enter a total the shop can store.".into()));
    }
    let mut remaining = body.total_amount;
    for payment in &body.payments {
        check_payment(payment, remaining)?;
        remaining -= payment.amount;
    }
    let mut tx = db.begin(false).await?;
    let id: i32 = sqlx::query_scalar(
        r#"
        INSERT INTO tailor_jobs (
            customer_name, phone, garment, due_on, measurements,
            cloth_type, cloth_color, cloth_quantity, cloth_source, cloth_notes,
            sewing_details, notes, total_amount, delivery_date
        )
        VALUES ($1, $2, $3, COALESCE($4, ''), $5, $6, $7, $8, $9, $10, $11, $12, $13, $4::date)
        RETURNING id
        "#,
    )
    .bind(&customer_name)
    .bind(clean(&body.phone, 40))
    .bind(&garment)
    .bind(delivery.as_deref())
    .bind(measurements_json(&body.measurements))
    .bind(clean(&body.cloth.kind, 40))
    .bind(clean(&body.cloth.color, 40))
    .bind(clean(&body.cloth.quantity, 20))
    .bind(&body.cloth.source)
    .bind(clean(&body.cloth.notes, 200))
    .bind(clean(&body.sewing_details, 500))
    .bind(clean(&body.notes, 500))
    .bind(body.total_amount)
    .fetch_one(tx.conn())
    .await?;
    for payment in &body.payments {
        sqlx::query(
            "INSERT INTO tailor_payments (job_id, amount, method, note) VALUES ($1, $2, $3, $4)",
        )
        .bind(id)
        .bind(payment.amount)
        .bind(&payment.method)
        .bind(clean(&payment.note, 80))
        .execute(tx.conn())
        .await?;
    }
    let job = job_json(&mut tx, id).await?;
    tx.commit().await?;
    Ok(Json(job))
}

async fn update_job(db: TenantDb, Path(id): Path<i32>, Json(body): Json<JobUpdate>) -> Result<Json<Value>, AppError> {
    check_status(&body.status, &body.delivery_status)?;
    let mut tx = db.begin(false).await?;
    let updated = sqlx::query("UPDATE tailor_jobs SET status = $1, delivery_status = $2 WHERE id = $3")
        .bind(&body.status)
        .bind(&body.delivery_status)
        .bind(id)
        .execute(tx.conn())
        .await?;
    if updated.rows_affected() == 0 {
        return Err(AppError::NotFound("That job is no longer saved.".into()));
    }
    let job = job_json(&mut tx, id).await?;
    tx.commit().await?;
    Ok(Json(job))
}

async fn add_payment(db: TenantDb, Path(id): Path<i32>, Json(body): Json<PaymentInput>) -> Result<Json<Value>, AppError> {
    let mut tx = db.begin(false).await?;
    let total: Option<i64> = sqlx::query_scalar("SELECT total_amount FROM tailor_jobs WHERE id = $1")
        .bind(id)
        .fetch_optional(tx.conn())
        .await?;
    let Some(total) = total else {
        return Err(AppError::NotFound("That job is no longer saved.".into()));
    };
    let already = paid(&mut tx, id).await?;
    check_payment(&body, total - already)?;
    sqlx::query("INSERT INTO tailor_payments (job_id, amount, method, note) VALUES ($1, $2, $3, $4)")
        .bind(id)
        .bind(body.amount)
        .bind(&body.method)
        .bind(clean(&body.note, 80))
        .execute(tx.conn())
        .await?;
    let job = job_json(&mut tx, id).await?;
    tx.commit().await?;
    Ok(Json(job))
}

pub fn mount(routes: &mut PluginRoutes) -> Result<(), RegistrationError> {
    routes.get("jobs", list, "tailor.read")?;
    routes.post("jobs", create, "tailor.write")?;
    routes.get("jobs/{id}", get_job, "tailor.read")?;
    routes.put("jobs/{id}", update_job, "tailor.write")?;
    routes.post("jobs/{id}/payments", add_payment, "tailor.write")?;
    Ok(())
}
