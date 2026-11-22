use axum::extract::Path;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use catalog_contract::CatalogQuery;
use platform_kernel::error::AppError;
use platform_kernel::routes::{PluginRoutes, RegistrationError, TenantDb};
use serde_json::{json, Value};

use crate::service::{self, CategoryForm, ItemForm, ItemPromptRequest, SectionForm, UnitForm};

struct Fail(AppError);

impl From<AppError> for Fail {
    fn from(value: AppError) -> Self {
        Fail(value)
    }
}

impl From<sqlx::Error> for Fail {
    fn from(value: sqlx::Error) -> Self {
        Fail(AppError::from(value))
    }
}

impl IntoResponse for Fail {
    fn into_response(self) -> axum::response::Response {
        let status = StatusCode::from_u16(self.0.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        (status, Json(json!({ "error": self.0.to_string() }))).into_response()
    }
}

async fn list_items(db: TenantDb) -> Result<Json<Value>, Fail> {
    let mut tx = db.begin(true).await?;
    let rows: Vec<(i32, String, i32, String)> = sqlx::query_as(
        "SELECT id, name, rate, item_type FROM items WHERE active ORDER BY sort_order, id",
    )
    .fetch_all(tx.conn())
    .await?;
    tx.commit().await?;
    Ok(Json(json!({
        "items": rows.into_iter().map(|(id, name, rate, item_type)| json!({
            "id": id, "name": name, "rate": rate, "item_type": item_type
        })).collect::<Vec<_>>()
    })))
}

async fn create_item(db: TenantDb, Json(form): Json<ItemForm>) -> Result<Json<Value>, Fail> {
    let mut tx = db.begin(false).await?;
    let id = service::save_item(&mut tx, None, &form).await?;
    tx.commit().await?;
    Ok(Json(json!({ "item_id": id })))
}

async fn list_units(db: TenantDb) -> Result<Json<Value>, Fail> {
    let mut tx = db.begin(true).await?;
    let units = service::list_units(&mut tx).await?;
    tx.commit().await?;
    Ok(Json(json!({ "units": units })))
}

async fn create_unit(db: TenantDb, Json(form): Json<UnitForm>) -> Result<Json<Value>, Fail> {
    let mut tx = db.begin(false).await?;
    let id = service::save_unit(&mut tx, &form).await?;
    tx.commit().await?;
    Ok(Json(json!({ "id": id })))
}

async fn list_categories(db: TenantDb) -> Result<Json<Value>, Fail> {
    let mut tx = db.begin(true).await?;
    let categories = service::Query.list_categories(&mut tx).await?;
    tx.commit().await?;
    Ok(Json(json!({ "categories": categories })))
}

async fn create_category(db: TenantDb, Json(form): Json<CategoryForm>) -> Result<Json<Value>, Fail> {
    let mut tx = db.begin(false).await?;
    let id = service::save_category(&mut tx, None, &form).await?;
    tx.commit().await?;
    Ok(Json(json!({ "id": id })))
}

async fn create_section(db: TenantDb, Json(form): Json<SectionForm>) -> Result<Json<Value>, Fail> {
    let mut tx = db.begin(false).await?;
    let id = service::save_section(&mut tx, None, &form).await?;
    tx.commit().await?;
    Ok(Json(json!({ "id": id })))
}

async fn item_prompt(
    db: TenantDb,
    Path(id): Path<i32>,
    Json(form): Json<ItemPromptRequest>,
) -> Result<Json<Value>, Fail> {
    let mut tx = db.begin(false).await?;
    service::set_item_prompt(&mut tx, id, &form).await?;
    tx.commit().await?;
    Ok(Json(json!({ "item_id": id })))
}

pub fn mount(routes: &mut PluginRoutes) -> Result<(), RegistrationError> {
    routes.get("items", list_items, "catalog.read")?;
    routes.post("items", create_item, "catalog.write")?;
    routes.get("units", list_units, "catalog.read")?;
    routes.post("units", create_unit, "catalog.write")?;
    routes.get("categories", list_categories, "catalog.read")?;
    routes.post("categories", create_category, "catalog.write")?;
    routes.post("sections", create_section, "catalog.write")?;
    routes.put("items/{id}/prompt", item_prompt, "catalog.write")?;
    Ok(())
}
