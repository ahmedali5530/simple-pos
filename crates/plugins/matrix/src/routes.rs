use axum::extract::Path;
use axum::Json;
use platform_kernel::error::AppError;
use platform_kernel::routes::{PluginRoutes, RegistrationError, TenantDb};
use serde_json::{json, Value};

use crate::service::{
    adjust_sku, clear_recipe, generate, get_recipe, list_enabled_item_ids, patch_sku, save_recipe,
    sku_summary, RecipeInput, SkuAdjust, SkuPatch,
};

async fn get_enabled(db: TenantDb) -> Result<Json<Value>, AppError> {
    let mut tx = db.begin(true).await?;
    let item_ids = list_enabled_item_ids(&mut tx).await?;
    tx.commit().await?;
    Ok(Json(json!({ "item_ids": item_ids })))
}

async fn get_item(db: TenantDb, Path(item_id): Path<i32>) -> Result<Json<Value>, AppError> {
    let mut tx = db.begin(true).await?;
    let body = get_recipe(&mut tx, item_id).await?;
    tx.commit().await?;
    Ok(Json(body))
}

async fn put_item(
    db: TenantDb,
    Path(item_id): Path<i32>,
    Json(body): Json<RecipeInput>,
) -> Result<Json<Value>, AppError> {
    let mut tx = db.begin(false).await?;
    let empty = body.groups.is_empty() && !body.enabled && body.barcode_prefix.trim().is_empty();
    let response = if empty {
        clear_recipe(&mut tx, item_id).await?;
        get_recipe(&mut tx, item_id).await?
    } else {
        save_recipe(&mut tx, item_id, &body).await?
    };
    tx.commit().await?;
    Ok(Json(response))
}

async fn post_generate(
    db: TenantDb,
    Path(item_id): Path<i32>,
) -> Result<Json<Value>, AppError> {
    let mut tx = db.begin(false).await?;
    let body = generate(&mut tx, item_id).await?;
    tx.commit().await?;
    Ok(Json(body))
}

async fn put_sku_route(
    db: TenantDb,
    Path(variant_id): Path<i32>,
    Json(body): Json<SkuPatch>,
) -> Result<Json<Value>, AppError> {
    let mut tx = db.begin(false).await?;
    let response = patch_sku(&mut tx, variant_id, &body).await?;
    tx.commit().await?;
    Ok(Json(response))
}

async fn post_adjust(
    db: TenantDb,
    Path(variant_id): Path<i32>,
    Json(body): Json<SkuAdjust>,
) -> Result<Json<Value>, AppError> {
    let mut tx = db.begin(false).await?;
    let response = adjust_sku(&mut tx, variant_id, &body).await?;
    tx.commit().await?;
    Ok(Json(response))
}

async fn get_summary(db: TenantDb) -> Result<Json<Value>, AppError> {
    let mut tx = db.begin(true).await?;
    let body = sku_summary(&mut tx).await?;
    tx.commit().await?;
    Ok(Json(body))
}

pub fn mount(routes: &mut PluginRoutes) -> Result<(), RegistrationError> {
    routes.get("enabled", get_enabled, "matrix.read")?;
    routes.get("items/{item_id}", get_item, "matrix.read")?;
    routes.put("items/{item_id}", put_item, "matrix.write")?;
    routes.post("items/{item_id}/generate", post_generate, "matrix.write")?;
    routes.put("skus/{variant_id}", put_sku_route, "matrix.write")?;
    routes.post("skus/{variant_id}/adjust", post_adjust, "matrix.write")?;
    routes.get("skus/summary", get_summary, "matrix.read")?;
    Ok(())
}
