use axum::extract::{Multipart, Path};
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use catalog_contract::{CatalogApi, CatalogQuery, CatalogSnapshot};
use platform_kernel::error::AppError;
use platform_kernel::routes::{PluginRoutes, RegistrationError, TenantDb};
use sales_contract::SalesQueryProvider;
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
        let status =
            StatusCode::from_u16(self.0.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        (status, Json(json!({ "error": self.0.to_string() }))).into_response()
    }
}

async fn snapshot(db: TenantDb) -> Result<Json<CatalogSnapshot>, Fail> {
    let catalog = db.provider::<CatalogApi>().ok_or_else(|| {
        AppError::Conflict("The catalog provider is not registered.".into())
    })?;
    let mut tx = db.begin(true).await?;
    let body = catalog.0.snapshot(&mut tx).await?;
    tx.commit().await?;
    Ok(Json(body))
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
    if let Some(ledger) = db.provider::<inventory_contract::StockLedgerProvider>() {
        ledger
            .0
            .ensure_balance(&mut tx, id, Some(form.cost))
            .await?;
    }
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

async fn create_category(
    db: TenantDb,
    Json(form): Json<CategoryForm>,
) -> Result<Json<Value>, Fail> {
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

fn workbook_response(filename: &str, bytes: Vec<u8>) -> Response {
    let disposition = HeaderValue::from_str(&format!("attachment; filename=\"{filename}\""))
        .unwrap_or_else(|_| HeaderValue::from_static("attachment; filename=\"workbook.xlsx\""));
    (
        StatusCode::OK,
        [
            (
                header::CONTENT_TYPE,
                HeaderValue::from_static(
                    "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
                ),
            ),
            (header::CONTENT_DISPOSITION, disposition),
        ],
        bytes,
    )
        .into_response()
}

async fn workbook_bytes(mut multipart: Multipart) -> Result<Vec<u8>, Fail> {
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| AppError::BadRequest("That workbook could not be read.".into()))?
    {
        if field.name() == Some("file") {
            let bytes = field
                .bytes()
                .await
                .map_err(|_| AppError::BadRequest("That workbook could not be read.".into()))?;
            if bytes.is_empty() {
                return Err(Fail(AppError::BadRequest(
                    "Choose an Excel workbook.".into(),
                )));
            }
            return Ok(bytes.to_vec());
        }
    }
    Err(Fail(AppError::BadRequest(
        "Choose an Excel workbook.".into(),
    )))
}

async fn export_items(db: TenantDb) -> Result<Response, Fail> {
    let mut tx = db.begin(true).await?;
    let bytes = crate::sheets::export_items(tx.transaction()).await?;
    tx.commit().await?;
    Ok(workbook_response("items.xlsx", bytes))
}

async fn import_items(db: TenantDb, multipart: Multipart) -> Result<Json<Value>, Fail> {
    let bytes = workbook_bytes(multipart).await?;
    let workbook = db.provider::<inventory_contract::InventoryWorkbookProvider>();
    let ledger = db.provider::<inventory_contract::StockLedgerProvider>();
    let mut tx = db.begin(false).await?;
    let imported =
        crate::sheets::import_items(&mut tx, workbook.as_deref(), ledger.as_deref(), &bytes)
            .await?;
    tx.commit().await?;
    Ok(Json(json!(imported)))
}

async fn export_categories(db: TenantDb) -> Result<Response, Fail> {
    let mut tx = db.begin(true).await?;
    let bytes = crate::sheets::export_categories(tx.transaction()).await?;
    tx.commit().await?;
    Ok(workbook_response("categories.xlsx", bytes))
}

async fn import_categories(db: TenantDb, multipart: Multipart) -> Result<Json<Value>, Fail> {
    let bytes = workbook_bytes(multipart).await?;
    let mut tx = db.begin(false).await?;
    let imported = crate::sheets::import_categories(tx.transaction(), &bytes).await?;
    tx.commit().await?;
    Ok(Json(json!(imported)))
}

async fn update_item(
    db: TenantDb,
    Path(id): Path<i32>,
    Json(form): Json<ItemForm>,
) -> Result<Json<Value>, Fail> {
    let mut tx = db.begin(false).await?;
    let id = service::save_item(&mut tx, Some(id), &form).await?;
    if let Some(ledger) = db.provider::<inventory_contract::StockLedgerProvider>() {
        ledger
            .0
            .ensure_balance(&mut tx, id, Some(form.cost))
            .await?;
    }
    tx.commit().await?;
    Ok(Json(json!({ "item_id": id })))
}

async fn update_section(
    db: TenantDb,
    Path(id): Path<i32>,
    Json(form): Json<SectionForm>,
) -> Result<Json<Value>, Fail> {
    let mut tx = db.begin(false).await?;
    let id = service::save_section(&mut tx, Some(id), &form).await?;
    tx.commit().await?;
    Ok(Json(json!({ "id": id })))
}

async fn delete_section(db: TenantDb, Path(id): Path<i32>) -> Result<Json<Value>, Fail> {
    let mut tx = db.begin(false).await?;
    let bill_count = if let Some(sales) = db.provider::<SalesQueryProvider>() {
        sales.0.bill_count_for_section(&mut tx, id).await?
    } else {
        0
    };
    service::remove_section(&mut tx, id, bill_count).await?;
    tx.commit().await?;
    Ok(Json(json!({ "id": id })))
}

#[derive(serde::Deserialize)]
struct IdOrder {
    ids: Vec<i32>,
}

async fn order_sections(db: TenantDb, Json(body): Json<IdOrder>) -> Result<Json<Value>, Fail> {
    let mut tx = db.begin(false).await?;
    service::reorder(&mut tx, "sections", "pos_order", &body.ids).await?;
    tx.commit().await?;
    Ok(Json(json!({ "ok": true })))
}

async fn update_category(
    db: TenantDb,
    Path(id): Path<i32>,
    Json(form): Json<CategoryForm>,
) -> Result<Json<Value>, Fail> {
    let mut tx = db.begin(false).await?;
    let id = service::save_category(&mut tx, Some(id), &form).await?;
    tx.commit().await?;
    Ok(Json(json!({ "id": id })))
}

async fn delete_category(db: TenantDb, Path(id): Path<i32>) -> Result<Json<Value>, Fail> {
    let mut tx = db.begin(false).await?;
    service::remove_category(&mut tx, id).await?;
    tx.commit().await?;
    Ok(Json(json!({ "id": id })))
}

async fn order_categories(db: TenantDb, Json(body): Json<IdOrder>) -> Result<Json<Value>, Fail> {
    let mut tx = db.begin(false).await?;
    service::reorder(&mut tx, "categories", "sort_order", &body.ids).await?;
    tx.commit().await?;
    Ok(Json(json!({ "ok": true })))
}

async fn read_image(db: TenantDb, Path(id): Path<i32>) -> Result<Response, Fail> {
    let mut tx = db.begin(true).await?;
    let row: Option<(Option<String>,)> =
        sqlx::query_as("SELECT image_ext FROM items WHERE id = $1 AND active")
            .bind(id)
            .fetch_optional(tx.conn())
            .await?;
    tx.commit().await?;
    let Some((Some(ext),)) = row else {
        return Err(Fail(AppError::NotFound("This item has no picture.".into())));
    };
    let Some(path) = crate::images::image_file(id, &ext) else {
        return Err(Fail(AppError::NotFound("This item has no picture.".into())));
    };
    let bytes =
        std::fs::read(&path).map_err(|_| AppError::NotFound("This item has no picture.".into()))?;
    Ok((
        StatusCode::OK,
        [
            (
                header::CONTENT_TYPE,
                HeaderValue::from_static(crate::images::content_type(&ext)),
            ),
            (
                header::CACHE_CONTROL,
                HeaderValue::from_static("public, max-age=31536000, immutable"),
            ),
        ],
        bytes,
    )
        .into_response())
}

async fn store_image(
    db: TenantDb,
    Path(id): Path<i32>,
    mut multipart: Multipart,
) -> Result<Json<Value>, Fail> {
    let mut tx = db.begin(true).await?;
    let previous: Option<(Option<String>,)> =
        sqlx::query_as("SELECT image_ext FROM items WHERE id = $1 AND active")
            .bind(id)
            .fetch_optional(tx.conn())
            .await?;
    tx.commit().await?;
    let mut picture = None;
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| AppError::BadRequest("That picture could not be read.".into()))?
    {
        if field.name() == Some("picture") {
            let data = field
                .bytes()
                .await
                .map_err(|_| AppError::BadRequest("That picture could not be read.".into()))?;
            picture = Some(data.to_vec());
            break;
        }
    }
    let Some(bytes) = picture.filter(|bytes| !bytes.is_empty()) else {
        return Err(Fail(AppError::BadRequest("Choose a picture.".into())));
    };
    let ext = crate::images::sniff_image(&bytes)?;
    crate::images::write_image(id, ext, &bytes)?;
    let mut tx = db.begin(false).await?;
    if let Err(error) = service::set_item_image(&mut tx, id, ext).await {
        crate::images::remove_image_file(id, ext);
        return Err(Fail(error));
    }
    tx.commit().await?;
    if let Some((Some(old),)) = previous {
        if old != ext {
            crate::images::remove_image_file(id, &old);
        }
    }
    Ok(Json(json!({ "item_id": id })))
}

async fn clear_image(db: TenantDb, Path(id): Path<i32>) -> Result<Json<Value>, Fail> {
    let mut tx = db.begin(false).await?;
    let previous: Option<(Option<String>,)> =
        sqlx::query_as("SELECT image_ext FROM items WHERE id = $1 AND active")
            .bind(id)
            .fetch_optional(tx.conn())
            .await?;
    service::clear_item_image(&mut tx, id).await?;
    tx.commit().await?;
    if let Some((Some(ext),)) = previous {
        crate::images::remove_image_file(id, &ext);
    }
    Ok(Json(json!({ "item_id": id })))
}

async fn export_movements(db: TenantDb) -> Result<Response, Fail> {
    let provider = db
        .provider::<inventory_contract::InventoryWorkbookProvider>()
        .ok_or_else(|| AppError::Conflict("Inventory must be enabled for movements.".into()))?;
    let mut tx = db.begin(true).await?;
    let rows = provider.0.movement_rows(&mut tx).await?;
    let bytes = crate::sheets::export_movements(&rows)?;
    tx.commit().await?;
    Ok(workbook_response("stock-movements.xlsx", bytes))
}

async fn import_movements(db: TenantDb, multipart: Multipart) -> Result<Json<Value>, Fail> {
    let provider = db
        .provider::<inventory_contract::InventoryWorkbookProvider>()
        .ok_or_else(|| AppError::Conflict("Inventory must be enabled for movements.".into()))?;
    let bytes = workbook_bytes(multipart).await?;
    let drafts = crate::sheets::parse_movement_workbook(&bytes)?;
    let mut tx = db.begin(false).await?;
    let imported = provider.0.apply_movements(&mut tx, &drafts).await?;
    tx.commit().await?;
    Ok(Json(json!({ "applied": imported.applied })))
}

pub fn mount(routes: &mut PluginRoutes) -> Result<(), RegistrationError> {
    routes.get("snapshot", snapshot, "catalog.read")?;
    routes.get("items", list_items, "catalog.read")?;
    routes.post("items", create_item, "catalog.write")?;
    routes.put("items/{id}", update_item, "catalog.write")?;
    routes.get("items/export", export_items, "catalog.read")?;
    routes.post("items/import", import_items, "catalog.write")?;
    routes.get("items/{id}/image", read_image, "catalog.read")?;
    routes.post("items/{id}/image", store_image, "catalog.write")?;
    routes.delete("items/{id}/image", clear_image, "catalog.write")?;
    routes.get("categories/export", export_categories, "catalog.read")?;
    routes.post("categories/import", import_categories, "catalog.write")?;
    routes.get("movements/export", export_movements, "catalog.read")?;
    routes.post("movements/import", import_movements, "catalog.write")?;
    routes.get("units", list_units, "catalog.read")?;
    routes.post("units", create_unit, "catalog.write")?;
    routes.get("categories", list_categories, "catalog.read")?;
    routes.post("categories", create_category, "catalog.write")?;
    routes.put("categories/order", order_categories, "catalog.write")?;
    routes.put("categories/{id}", update_category, "catalog.write")?;
    routes.delete("categories/{id}", delete_category, "catalog.write")?;
    routes.post("sections", create_section, "catalog.write")?;
    routes.put("sections/order", order_sections, "catalog.write")?;
    routes.put("sections/{id}", update_section, "catalog.write")?;
    routes.delete("sections/{id}", delete_section, "catalog.write")?;
    routes.put("items/{id}/prompt", item_prompt, "catalog.write")?;
    Ok(())
}
