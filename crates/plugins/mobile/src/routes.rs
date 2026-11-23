use axum::extract::{Path, Query};
use axum::Json;
use platform_kernel::error::AppError;
use platform_kernel::routes::{PluginRoutes, RegistrationError, TenantDb};
use serde::Deserialize;
use serde_json::{json, Value};

const CLAIM_STATUSES: &[&str] = &["open", "in_progress", "resolved", "rejected"];
const CLAIM_KINDS: &[&str] = &["repair", "replacement"];
const DEVICE_STATUSES: &[&str] = &["in_stock", "sold"];

#[derive(sqlx::FromRow)]
struct ModelRow {
    item_id: i32,
    brand: String,
    model: String,
    warranty_months: i32,
}

#[derive(sqlx::FromRow)]
struct DeviceRow {
    id: i32,
    item_id: i32,
    imei: String,
    serial: String,
    status: String,
    received_on: String,
    warranty_months: i32,
    sold_bill_no: String,
    sold_at: Option<String>,
    customer_name: String,
    brand: String,
    model: String,
    starts_on: Option<String>,
    ends_on: Option<String>,
}

#[derive(sqlx::FromRow)]
struct ClaimRow {
    id: i32,
    device_id: i32,
    kind: String,
    status: String,
    problem: String,
    resolution: String,
    cost: i64,
    opened_on: String,
    closed_on: Option<String>,
    imei: String,
    serial: String,
    brand: String,
    model: String,
    warranty_ends_on: Option<String>,
}

fn model_json(row: ModelRow) -> Value {
    json!({
        "item_id": row.item_id,
        "brand": row.brand,
        "model": row.model,
        "warranty_months": row.warranty_months,
    })
}

fn device_json(row: DeviceRow) -> Value {
    json!({
        "id": row.id,
        "item_id": row.item_id,
        "imei": row.imei,
        "serial": row.serial,
        "status": row.status,
        "received_on": row.received_on,
        "warranty_months": row.warranty_months,
        "sold_bill_no": row.sold_bill_no,
        "sold_at": row.sold_at,
        "customer_name": row.customer_name,
        "brand": row.brand,
        "model": row.model,
        "starts_on": row.starts_on,
        "ends_on": row.ends_on,
    })
}

fn claim_json(row: ClaimRow) -> Value {
    json!({
        "id": row.id,
        "device_id": row.device_id,
        "kind": row.kind,
        "status": row.status,
        "problem": row.problem,
        "resolution": row.resolution,
        "cost": row.cost,
        "opened_on": row.opened_on,
        "closed_on": row.closed_on,
        "imei": row.imei,
        "serial": row.serial,
        "brand": row.brand,
        "model": row.model,
        "warranty_ends_on": row.warranty_ends_on,
    })
}

fn clean(value: &str, field: &str, max: usize) -> Result<String, AppError> {
    let trimmed = value.trim();
    if trimmed.chars().count() > max {
        return Err(AppError::BadRequest(format!(
            "Keep the {field} to {max} characters."
        )));
    }
    Ok(trimmed.to_string())
}

fn valid_months(value: i32) -> Result<i32, AppError> {
    if (1..=120).contains(&value) {
        Ok(value)
    } else {
        Err(AppError::BadRequest(
            "Warranty months must be between 1 and 120.".into(),
        ))
    }
}

fn valid_status(value: &str, allowed: &[&str], field: &str) -> Result<String, AppError> {
    let value = value.trim();
    if allowed.contains(&value) {
        Ok(value.to_string())
    } else {
        Err(AppError::BadRequest(format!("Choose a valid {field}.")))
    }
}

fn optional_filter(value: &str, allowed: &[&str], field: &str) -> Result<String, AppError> {
    if value.trim().is_empty() {
        return Ok(String::new());
    }
    valid_status(value, allowed, field)
}

fn search_pattern(value: &str) -> String {
    let term = value.trim();
    if term.is_empty() {
        String::new()
    } else {
        format!("%{term}%")
    }
}

async fn list_models(db: TenantDb) -> Result<Json<Value>, AppError> {
    let mut tx = db.begin(true).await?;
    let rows: Vec<ModelRow> = sqlx::query_as(
        "SELECT item_id, brand, model, warranty_months FROM mobile_models ORDER BY brand, model, item_id",
    )
    .fetch_all(tx.conn())
    .await?;
    tx.commit().await?;
    Ok(Json(json!({
        "models": rows.into_iter().map(model_json).collect::<Vec<_>>(),
    })))
}

async fn get_model(db: TenantDb, Path(item_id): Path<i32>) -> Result<Json<Value>, AppError> {
    let mut tx = db.begin(true).await?;
    let row: Option<ModelRow> = sqlx::query_as(
        "SELECT item_id, brand, model, warranty_months FROM mobile_models WHERE item_id = $1",
    )
    .bind(item_id)
    .fetch_optional(tx.conn())
    .await?;
    tx.commit().await?;
    Ok(Json(json!({ "model": row.map(model_json) })))
}

#[derive(Deserialize)]
struct ModelInput {
    #[serde(default)]
    brand: String,
    #[serde(default)]
    model: String,
    #[serde(default = "default_months")]
    warranty_months: i32,
}

fn default_months() -> i32 {
    12
}

async fn save_model(
    db: TenantDb,
    Path(item_id): Path<i32>,
    Json(body): Json<ModelInput>,
) -> Result<Json<Value>, AppError> {
    let brand = clean(&body.brand, "brand", 80)?;
    let model = clean(&body.model, "model", 120)?;
    if brand.is_empty() && model.is_empty() {
        return Err(AppError::BadRequest(
            "Enter a brand or model for this item.".into(),
        ));
    }
    let months = valid_months(body.warranty_months)?;
    let mut tx = db.begin(false).await?;
    let exists: Option<i32> = sqlx::query_scalar("SELECT id FROM items WHERE id = $1 AND active")
        .bind(item_id)
        .fetch_optional(tx.conn())
        .await?;
    if exists.is_none() {
        return Err(AppError::NotFound(
            "That item is not in the catalog.".into(),
        ));
    }
    let row: ModelRow = sqlx::query_as(
        r#"
        INSERT INTO mobile_models (item_id, brand, model, warranty_months)
        VALUES ($1, $2, $3, $4)
        ON CONFLICT (tenant_id, item_id) DO UPDATE
        SET brand = EXCLUDED.brand,
            model = EXCLUDED.model,
            warranty_months = EXCLUDED.warranty_months,
            updated_at = now()
        RETURNING item_id, brand, model, warranty_months
        "#,
    )
    .bind(item_id)
    .bind(&brand)
    .bind(&model)
    .bind(months)
    .fetch_one(tx.conn())
    .await?;
    tx.commit().await?;
    Ok(Json(json!({ "model": model_json(row) })))
}

async fn clear_model(db: TenantDb, Path(item_id): Path<i32>) -> Result<Json<Value>, AppError> {
    let mut tx = db.begin(false).await?;
    sqlx::query("DELETE FROM mobile_models WHERE item_id = $1")
        .bind(item_id)
        .execute(tx.conn())
        .await?;
    tx.commit().await?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
struct DeviceQuery {
    #[serde(default)]
    status: String,
    #[serde(default)]
    search: String,
    #[serde(default)]
    item_id: Option<i32>,
}

/// Device status is derived from the bills too, not only from `mobile_devices`.
/// The warranty handler marks the row sold a moment after the bill commits, so
/// between those two steps a just-sold unit would otherwise still look available.
const DEVICE_SQL: &str = r#"
    WITH devices AS (
        SELECT d.id, d.item_id, d.imei, d.serial,
               CASE
                   WHEN d.status = 'sold' OR sold.bill_id IS NOT NULL THEN 'sold'
                   ELSE 'in_stock'
               END AS status,
               d.received_on::text AS received_on,
               d.warranty_months,
               COALESCE(NULLIF(d.sold_bill_no, ''), sold.bill_no, '') AS sold_bill_no,
               COALESCE(d.sold_at::text, sold.created_at) AS sold_at,
               COALESCE(NULLIF(d.customer_name, ''), sold.customer, '') AS customer_name,
               COALESCE(m.brand, '') AS brand,
               COALESCE(m.model, '') AS model,
               w.starts_on,
               w.ends_on
        FROM mobile_devices d
        LEFT JOIN LATERAL (
            SELECT b.id AS bill_id,
                   b.bill_no,
                   b.created_at::text AS created_at,
                   COALESCE(b.customer, '') AS customer
            FROM bill_lines bl
            JOIN bills b ON b.tenant_id = bl.tenant_id AND b.id = bl.bill_id
            WHERE bl.tenant_id = d.tenant_id
              AND bl.source = 'mobile.device'
              AND bl.reference = 'device:' || d.id::text
            ORDER BY bl.id DESC
            LIMIT 1
        ) sold ON true
        LEFT JOIN mobile_models m
          ON m.tenant_id = d.tenant_id AND m.item_id = d.item_id
        LEFT JOIN LATERAL (
            SELECT starts_on::text AS starts_on, ends_on::text AS ends_on
            FROM mobile_warranties
            WHERE tenant_id = d.tenant_id AND device_id = d.id
            ORDER BY id DESC
            LIMIT 1
        ) w ON true
    )
"#;

async fn list_devices(
    db: TenantDb,
    Query(query): Query<DeviceQuery>,
) -> Result<Json<Value>, AppError> {
    let status = optional_filter(&query.status, DEVICE_STATUSES, "device status")?;
    let search = search_pattern(&query.search);
    let mut tx = db.begin(true).await?;
    let rows: Vec<DeviceRow> = sqlx::query_as(&format!(
        "{DEVICE_SQL}
        SELECT * FROM devices
        WHERE ($1::text = '' OR status = $1)
          AND ($2::text = ''
               OR imei ILIKE $2
               OR serial ILIKE $2
               OR brand ILIKE $2
               OR model ILIKE $2)
          AND ($3::int IS NULL OR item_id = $3)
        ORDER BY id DESC
        LIMIT 200"
    ))
    .bind(&status)
    .bind(&search)
    .bind(query.item_id)
    .fetch_all(tx.conn())
    .await?;
    let (in_stock, sold): (i64, i64) = sqlx::query_as(&format!(
        "{DEVICE_SQL}
        SELECT COUNT(*) FILTER (WHERE status = 'in_stock'),
               COUNT(*) FILTER (WHERE status = 'sold')
        FROM devices"
    ))
    .fetch_one(tx.conn())
    .await?;
    tx.commit().await?;
    Ok(Json(json!({
        "devices": rows.into_iter().map(device_json).collect::<Vec<_>>(),
        "in_stock": in_stock,
        "sold": sold,
    })))
}

#[derive(Deserialize)]
struct NewDevice {
    item_id: i32,
    imei: String,
    #[serde(default)]
    serial: String,
    #[serde(default)]
    warranty_months: Option<i32>,
}

async fn add_device(db: TenantDb, Json(body): Json<NewDevice>) -> Result<Json<Value>, AppError> {
    let imei = clean(&body.imei, "IMEI", 32)?;
    if imei.is_empty() {
        return Err(AppError::BadRequest("Enter the IMEI.".into()));
    }
    let serial = clean(&body.serial, "serial", 64)?;
    let mut tx = db.begin(false).await?;
    let model_months: Option<i32> =
        sqlx::query_scalar("SELECT warranty_months FROM mobile_models WHERE item_id = $1")
            .bind(body.item_id)
            .fetch_optional(tx.conn())
            .await?;
    let Some(model_months) = model_months else {
        return Err(AppError::BadRequest(
            "Add the mobile brand and model to that catalog item first.".into(),
        ));
    };
    let months = valid_months(body.warranty_months.unwrap_or(model_months))?;
    let id: Option<i32> = sqlx::query_scalar(
        r#"
        INSERT INTO mobile_devices (item_id, imei, serial, warranty_months)
        VALUES ($1, $2, $3, $4)
        ON CONFLICT (tenant_id, imei) DO NOTHING
        RETURNING id
        "#,
    )
    .bind(body.item_id)
    .bind(&imei)
    .bind(&serial)
    .bind(months)
    .fetch_optional(tx.conn())
    .await?;
    let Some(id) = id else {
        return Err(AppError::Conflict(
            "That IMEI is already registered.".into(),
        ));
    };
    // The device registry owns phone availability. Device lines are
    // stock-neutral, so registering one does not touch the catalog quantity.
    tx.commit().await?;
    Ok(Json(json!({ "id": id })))
}

#[derive(Deserialize)]
struct ClaimQuery {
    #[serde(default)]
    status: String,
    #[serde(default)]
    device_id: Option<i32>,
    #[serde(default)]
    search: String,
}

const CLAIM_SQL: &str = r#"
    SELECT c.id, c.device_id, c.kind, c.status, c.problem, c.resolution, c.cost,
           c.opened_on::text AS opened_on,
           c.closed_on::text AS closed_on,
           d.imei, d.serial,
           COALESCE(m.brand, '') AS brand,
           COALESCE(m.model, '') AS model,
           w.ends_on::text AS warranty_ends_on
    FROM mobile_claims c
    JOIN mobile_devices d
      ON d.tenant_id = c.tenant_id AND d.id = c.device_id
    LEFT JOIN mobile_models m
      ON m.tenant_id = d.tenant_id AND m.item_id = d.item_id
    LEFT JOIN mobile_warranties w
      ON w.tenant_id = d.tenant_id AND w.device_id = d.id
"#;

async fn list_claims(
    db: TenantDb,
    Query(query): Query<ClaimQuery>,
) -> Result<Json<Value>, AppError> {
    let status = optional_filter(&query.status, CLAIM_STATUSES, "claim status")?;
    let search = search_pattern(&query.search);
    let mut tx = db.begin(true).await?;
    let rows: Vec<ClaimRow> = sqlx::query_as(&format!(
        "{CLAIM_SQL}
        WHERE ($1::text = '' OR c.status = $1)
          AND ($2::int IS NULL OR c.device_id = $2)
          AND ($3::text = ''
               OR d.imei ILIKE $3
               OR d.serial ILIKE $3
               OR COALESCE(m.brand, '') ILIKE $3
               OR COALESCE(m.model, '') ILIKE $3)
        ORDER BY c.id DESC
        LIMIT 200"
    ))
    .bind(&status)
    .bind(query.device_id)
    .bind(&search)
    .fetch_all(tx.conn())
    .await?;
    tx.commit().await?;
    Ok(Json(json!({
        "claims": rows.into_iter().map(claim_json).collect::<Vec<_>>(),
    })))
}

#[derive(Deserialize)]
struct NewClaim {
    device_id: i32,
    kind: String,
    #[serde(default)]
    problem: String,
    #[serde(default)]
    cost: i64,
}

async fn open_claim(db: TenantDb, Json(body): Json<NewClaim>) -> Result<Json<Value>, AppError> {
    let kind = valid_status(&body.kind, CLAIM_KINDS, "claim kind")?;
    let problem = clean(&body.problem, "problem", 500)?;
    if problem.is_empty() {
        return Err(AppError::BadRequest("Describe the problem.".into()));
    }
    if body.cost < 0 {
        return Err(AppError::BadRequest("Cost cannot be negative.".into()));
    }
    let mut tx = db.begin(false).await?;
    let device: Option<i32> = sqlx::query_scalar("SELECT id FROM mobile_devices WHERE id = $1")
        .bind(body.device_id)
        .fetch_optional(tx.conn())
        .await?;
    if device.is_none() {
        return Err(AppError::NotFound(
            "That device is not registered.".into(),
        ));
    }
    let id: i32 = sqlx::query_scalar(
        r#"
        INSERT INTO mobile_claims (device_id, kind, problem, cost, opened_on)
        VALUES ($1, $2, $3, $4,
                (now() AT TIME ZONE COALESCE((SELECT timezone FROM tenant_profile LIMIT 1), 'UTC'))::date)
        RETURNING id
        "#,
    )
    .bind(body.device_id)
    .bind(&kind)
    .bind(&problem)
    .bind(body.cost)
    .fetch_one(tx.conn())
    .await?;
    let row: ClaimRow = sqlx::query_as(&format!("{CLAIM_SQL} WHERE c.id = $1"))
        .bind(id)
        .fetch_one(tx.conn())
        .await?;
    tx.commit().await?;
    Ok(Json(json!({ "claim": claim_json(row) })))
}

#[derive(Deserialize)]
struct ClaimUpdate {
    status: String,
    #[serde(default)]
    resolution: Option<String>,
    #[serde(default)]
    cost: Option<i64>,
}

async fn update_claim(
    db: TenantDb,
    Path(id): Path<i32>,
    Json(body): Json<ClaimUpdate>,
) -> Result<Json<Value>, AppError> {
    let status = valid_status(&body.status, CLAIM_STATUSES, "claim status")?;
    let resolution = body
        .resolution
        .as_deref()
        .map(|value| clean(value, "resolution", 500))
        .transpose()?;
    if let Some(cost) = body.cost {
        if cost < 0 {
            return Err(AppError::BadRequest("Cost cannot be negative.".into()));
        }
    }
    let mut tx = db.begin(false).await?;
    let updated = sqlx::query(
        r#"
        UPDATE mobile_claims
        SET status = $1,
            resolution = COALESCE($2, resolution),
            cost = COALESCE($3, cost),
            closed_on = CASE
                WHEN $1 IN ('resolved', 'rejected')
                THEN COALESCE(closed_on,
                    (now() AT TIME ZONE COALESCE((SELECT timezone FROM tenant_profile LIMIT 1), 'UTC'))::date)
                ELSE NULL
            END,
            updated_at = now()
        WHERE id = $4
        RETURNING id
        "#,
    )
    .bind(&status)
    .bind(&resolution)
    .bind(body.cost)
    .bind(id)
    .fetch_optional(tx.conn())
    .await?;
    if updated.is_none() {
        return Err(AppError::NotFound("That claim is not saved.".into()));
    }
    let row: ClaimRow = sqlx::query_as(&format!("{CLAIM_SQL} WHERE c.id = $1"))
        .bind(id)
        .fetch_one(tx.conn())
        .await?;
    tx.commit().await?;
    Ok(Json(json!({ "claim": claim_json(row) })))
}

pub fn mount(routes: &mut PluginRoutes) -> Result<(), RegistrationError> {
    routes.get("models", list_models, "mobile.read")?;
    routes.get("models/{item_id}", get_model, "mobile.read")?;
    routes.put("models/{item_id}", save_model, "mobile.write")?;
    routes.delete("models/{item_id}", clear_model, "mobile.write")?;
    routes.get("devices", list_devices, "mobile.read")?;
    routes.post("devices", add_device, "mobile.write")?;
    routes.get("claims", list_claims, "mobile.read")?;
    routes.post("claims", open_claim, "mobile.write")?;
    routes.put("claims/{id}", update_claim, "mobile.write")?;
    Ok(())
}
