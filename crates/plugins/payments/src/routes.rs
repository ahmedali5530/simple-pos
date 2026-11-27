use axum::extract::Query;
use axum::response::Response;
use axum::Json;
use platform_kernel::error::AppError;
use platform_kernel::routes::{PluginRoutes, RegistrationError, TenantDb};
use serde::{Deserialize, Serialize};

use crate::export::{self, ExportFormat};
use crate::settings::{self, PaymentsSettings};

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct MethodTotal {
    pub method: String,
    pub label: String,
    pub amount: i64,
    pub bills: i64,
}

#[derive(Debug, Deserialize)]
struct ReportQuery {
    from: Option<String>,
    to: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ExportQuery {
    format: String,
    from: Option<String>,
    to: Option<String>,
}

#[derive(Debug, Serialize)]
struct PaymentsReport {
    totals: Vec<MethodTotal>,
    collected: i64,
}

pub fn mount(routes: &mut PluginRoutes) -> Result<(), RegistrationError> {
    routes.get("settings", get_settings, "payments.read")?;
    routes.put("settings", put_settings, "payments.write")?;
    routes.get("types", list_types, "payments.read")?;
    routes.get("report", report, "payments.read")?;
    routes.get("export", export_report, "payments.read")?;
    Ok(())
}

async fn get_settings(db: TenantDb) -> Result<Json<PaymentsSettings>, AppError> {
    let mut tx = db.begin(true).await?;
    let settings = settings::load(&mut tx).await?;
    tx.commit().await?;
    Ok(Json(settings))
}

async fn put_settings(
    db: TenantDb,
    Json(body): Json<PaymentsSettings>,
) -> Result<Json<PaymentsSettings>, AppError> {
    let mut tx = db.begin(false).await?;
    settings::save(&mut tx, &body).await?;
    let saved = settings::load(&mut tx).await?;
    tx.commit().await?;
    Ok(Json(saved))
}

async fn list_types(db: TenantDb) -> Result<Json<PaymentsSettings>, AppError> {
    get_settings(db).await
}

async fn load_totals(
    db: &TenantDb,
    from: Option<&str>,
    to: Option<&str>,
) -> Result<(Vec<MethodTotal>, i64), AppError> {
    let mut tx = db.begin(true).await?;
    let totals = sqlx::query_as::<_, MethodTotal>(
        r#"
        SELECT method,
               MAX(label) AS label,
               COALESCE(SUM(amount), 0)::bigint AS amount,
               COUNT(DISTINCT bill_id)::bigint AS bills
        FROM payment_entries
        WHERE ($1::text IS NULL OR posted_at >= $1::timestamptz)
          AND ($2::text IS NULL OR posted_at < $2::timestamptz)
        GROUP BY method
        ORDER BY amount DESC, method
        "#,
    )
    .bind(from)
    .bind(to)
    .fetch_all(tx.conn())
    .await?;
    tx.commit().await?;
    let collected = totals.iter().map(|row| row.amount).sum();
    Ok((totals, collected))
}

async fn report(
    db: TenantDb,
    Query(query): Query<ReportQuery>,
) -> Result<Json<PaymentsReport>, AppError> {
    let (totals, collected) = load_totals(&db, query.from.as_deref(), query.to.as_deref()).await?;
    Ok(Json(PaymentsReport { totals, collected }))
}

async fn export_report(
    db: TenantDb,
    Query(query): Query<ExportQuery>,
) -> Result<Response, AppError> {
    let format = ExportFormat::parse(&query.format)?;
    let (totals, collected) = load_totals(&db, query.from.as_deref(), query.to.as_deref()).await?;
    let range_label = if query.from.is_some() && query.to.is_some() {
        "Selected range"
    } else {
        "All dates"
    };
    let bytes = export::build(format, &totals, collected, range_label)?;
    Ok(export::file_response(
        &format!("payment-mix.{}", format.extension()),
        format.content_type(),
        bytes,
    ))
}
