use axum::extract::Query;
use axum::Json;
use platform_kernel::error::AppError;
use platform_kernel::routes::{PluginRoutes, RegistrationError, TenantDb};
use serde::Deserialize;
use plugin_inventory::ledger::{load_reports, Reports};

#[derive(Deserialize)]
struct ReportQuery {
    from: Option<String>,
    to: Option<String>,
}

async fn summary(db: TenantDb, Query(query): Query<ReportQuery>) -> Result<Json<Reports>, AppError> {
    let mut tx = db.begin(true).await?;
    let report = load_reports(tx.transaction(), query.from.as_deref(), query.to.as_deref()).await?;
    tx.commit().await?;
    Ok(Json(report))
}

pub fn mount(routes: &mut PluginRoutes) -> Result<(), RegistrationError> {
    routes.get("summary", summary, "reports.read")?;
    Ok(())
}
