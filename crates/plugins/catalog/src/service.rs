use catalog_contract::{
    CatalogCategory, CatalogItem, CatalogSection, CatalogSnapshot, CatalogVariant, CategoryRef,
    ItemRef, SectionRef, VariantRef,
};
use platform_kernel::context::TenantTx;
use platform_kernel::error::AppError;
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Debug, Deserialize)]
pub struct SectionForm {
    pub name: String,
    pub short_name: String,
    pub kind: String,
    pub accent: String,
}

#[derive(Debug, Deserialize)]
pub struct CategoryForm {
    pub name: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct AttributeInput {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct VariantInput {
    #[serde(default)]
    pub id: Option<i32>,
    pub name: String,
    pub rate: i32,
    pub units: i32,
    #[serde(default)]
    pub barcode: Option<String>,
    #[serde(default)]
    pub unit_id: Option<i32>,
    #[serde(default)]
    pub attributes: Vec<AttributeInput>,
}

#[derive(Debug, Deserialize)]
pub struct ItemForm {
    pub name: String,
    #[serde(default)]
    pub barcode: Option<String>,
    pub rate: i32,
    #[serde(default)]
    pub cost: i64,
    pub section_id: i32,
    pub category_id: i32,
    pub prompt: String,
    #[serde(default)]
    pub base_rate: Option<i32>,
    pub scale: i32,
    #[serde(default)]
    pub variants: Vec<VariantInput>,
    #[serde(default = "product_type")]
    pub item_type: String,
    #[serde(default)]
    pub unit_id: Option<i32>,
}

fn product_type() -> String {
    "product".into()
}

#[derive(Debug, Deserialize)]
pub struct ItemPromptRequest {
    pub prompt: String,
    pub base_rate: Option<i32>,
    pub scale: i32,
}

#[derive(Debug, Deserialize)]
pub struct UnitForm {
    pub code: String,
    pub name: String,
    #[serde(default = "piece")]
    pub base_code: String,
    #[serde(default = "one")]
    pub factor: i32,
}

fn piece() -> String {
    "pc".into()
}
fn one() -> i32 {
    1
}

#[derive(Debug, Serialize)]
pub struct UnitDto {
    pub id: i32,
    pub code: String,
    pub name: String,
    pub base_code: String,
    pub factor: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemKind {
    Product,
    Service,
}

impl ItemKind {
    pub fn parse(value: &str) -> Result<Self, AppError> {
        match value {
            "product" => Ok(Self::Product),
            "service" => Ok(Self::Service),
            _ => Err(AppError::BadRequest("Choose product or service.".into())),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Product => "product",
            Self::Service => "service",
        }
    }

    pub fn tracks_stock(self) -> bool {
        matches!(self, Self::Product)
    }
}

pub fn tracks_stock(item_type: &str) -> bool {
    ItemKind::parse(item_type)
        .map(|kind| kind.tracks_stock())
        .unwrap_or(true)
}

#[cfg(test)]
mod form_tests {
    use super::*;

    #[test]
    fn old_item_payload_stays_a_product_without_new_columns() {
        let form: ItemForm = serde_json::from_value(json!({
            "name": "Tea",
            "rate": 10,
            "cost": 0,
            "section_id": 1,
            "category_id": 1,
            "prompt": "none",
            "scale": 1
        }))
        .unwrap();
        assert_eq!(form.item_type, "product");
        assert!(form.unit_id.is_none());
        assert!(form.variants.is_empty());
    }
}

fn clean_code(value: &Option<String>) -> Option<String> {
    value
        .as_ref()
        .map(|code| code.trim().to_string())
        .filter(|code| !code.is_empty())
}

fn catalog_error(err: sqlx::Error) -> AppError {
    let constraint = err
        .as_database_error()
        .and_then(|db| db.constraint().map(str::to_string));
    match constraint.as_deref() {
        Some("items_barcode_key") | Some("item_variants_barcode_key") => {
            AppError::BadRequest("That barcode is already used.".into())
        }
        Some("item_variants_item_id_name_key") => {
            AppError::BadRequest("Each option needs its own name.".into())
        }
        Some("categories_name_key") | Some("categories_tenant_id_name_key") => {
            AppError::BadRequest("That category name is already used.".into())
        }
        _ => AppError::Db(err),
    }
}

async fn emit(tx: &mut TenantTx<'_>, event_type: &str, item_id: i32) -> Result<(), AppError> {
    sqlx::query(
        r#"
        INSERT INTO platform_events (tenant_id, event_type, version, payload)
        VALUES (current_setting('app.tenant_id')::uuid, $1, 1, $2)
        "#,
    )
    .bind(event_type)
    .bind(json!({ "item_id": item_id }))
    .execute(tx.conn())
    .await?;
    Ok(())
}

pub async fn save_section(
    tx: &mut TenantTx<'_>,
    id: Option<i32>,
    form: &SectionForm,
) -> Result<i32, AppError> {
    let name = form.name.trim();
    let short_name = form.short_name.trim();
    if name.is_empty() || short_name.is_empty() {
        return Err(AppError::BadRequest(
            "Enter a section name and short name.".into(),
        ));
    }
    if form.kind != "retail" && form.kind != "food" {
        return Err(AppError::BadRequest("Choose retail or food.".into()));
    }
    if let Some(id) = id {
        let updated = sqlx::query(
            "UPDATE sections SET name = $1, short_name = $2, kind = $3, accent = $4 WHERE id = $5",
        )
        .bind(name)
        .bind(short_name)
        .bind(&form.kind)
        .bind(form.accent.trim())
        .bind(id)
        .execute(tx.conn())
        .await?;
        if updated.rows_affected() == 0 {
            return Err(AppError::NotFound(
                "That section is no longer in the catalog.".into(),
            ));
        }
        Ok(id)
    } else {
        let order =
            sqlx::query_scalar::<_, i32>("SELECT COALESCE(MAX(pos_order), -1) + 1 FROM sections")
                .fetch_one(tx.conn())
                .await?;
        let id = sqlx::query_scalar::<_, i32>(
            r#"
            INSERT INTO sections (name, short_name, kind, accent, show_on_pos, pos_order)
            VALUES ($1, $2, $3, $4, TRUE, $5)
            RETURNING id
            "#,
        )
        .bind(name)
        .bind(short_name)
        .bind(&form.kind)
        .bind(form.accent.trim())
        .bind(order)
        .fetch_one(tx.conn())
        .await?;
        Ok(id)
    }
}

pub async fn remove_section(
    tx: &mut TenantTx<'_>,
    id: i32,
    bill_count: i64,
) -> Result<(), AppError> {
    let items: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM items WHERE section_id = $1")
        .bind(id)
        .fetch_one(tx.conn())
        .await?;
    if items > 0 {
        return Err(AppError::BadRequest(
            "Move this section's items before removing it.".into(),
        ));
    }
    if bill_count > 0 {
        return Err(AppError::BadRequest(
            "This section has bills, so it stays on record.".into(),
        ));
    }
    let deleted = sqlx::query("DELETE FROM sections WHERE id = $1")
        .bind(id)
        .execute(tx.conn())
        .await?;
    if deleted.rows_affected() == 0 {
        return Err(AppError::NotFound(
            "That section is no longer in the catalog.".into(),
        ));
    }
    Ok(())
}

pub async fn save_category(
    tx: &mut TenantTx<'_>,
    id: Option<i32>,
    form: &CategoryForm,
) -> Result<i32, AppError> {
    let name = form.name.trim();
    if name.is_empty() {
        return Err(AppError::BadRequest("Enter a category name.".into()));
    }
    if let Some(id) = id {
        let updated = sqlx::query("UPDATE categories SET name = $1 WHERE id = $2")
            .bind(name)
            .bind(id)
            .execute(tx.conn())
            .await
            .map_err(catalog_error)?;
        if updated.rows_affected() == 0 {
            return Err(AppError::NotFound(
                "That category is no longer in the catalog.".into(),
            ));
        }
        Ok(id)
    } else {
        let order = sqlx::query_scalar::<_, i32>(
            "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM categories",
        )
        .fetch_one(tx.conn())
        .await?;
        sqlx::query_scalar("INSERT INTO categories (name, sort_order) VALUES ($1, $2) RETURNING id")
            .bind(name)
            .bind(order)
            .fetch_one(tx.conn())
            .await
            .map_err(catalog_error)
    }
}

pub async fn reorder(
    tx: &mut TenantTx<'_>,
    table: &str,
    column: &str,
    ids: &[i32],
) -> Result<(), AppError> {
    let (table, column) = match (table, column) {
        ("sections", "pos_order") => ("sections", "pos_order"),
        ("categories", "sort_order") => ("categories", "sort_order"),
        _ => {
            return Err(AppError::BadRequest(
                "That catalog list cannot be reordered.".into(),
            ))
        }
    };
    if ids.is_empty() {
        return Err(AppError::BadRequest(
            "List every record when changing the order.".into(),
        ));
    }
    let total: i64 = sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {table}"))
        .fetch_one(tx.conn())
        .await?;
    if total != ids.len() as i64 {
        return Err(AppError::BadRequest(
            "List every record when changing the order.".into(),
        ));
    }
    for (index, id) in ids.iter().enumerate() {
        sqlx::query(&format!("UPDATE {table} SET {column} = $1 WHERE id = $2"))
            .bind(index as i32)
            .bind(id)
            .execute(tx.conn())
            .await?;
    }
    Ok(())
}

pub async fn remove_category(tx: &mut TenantTx<'_>, id: i32) -> Result<(), AppError> {
    let items: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM items WHERE category_id = $1")
        .bind(id)
        .fetch_one(tx.conn())
        .await?;
    if items > 0 {
        return Err(AppError::BadRequest(
            "Move this category's items before removing it.".into(),
        ));
    }
    let deleted = sqlx::query("DELETE FROM categories WHERE id = $1")
        .bind(id)
        .execute(tx.conn())
        .await?;
    if deleted.rows_affected() == 0 {
        return Err(AppError::NotFound(
            "That category is no longer in the catalog.".into(),
        ));
    }
    Ok(())
}

pub async fn save_item(
    tx: &mut TenantTx<'_>,
    item_id: Option<i32>,
    form: &ItemForm,
) -> Result<i32, AppError> {
    let name = form.name.trim();
    if name.is_empty() {
        return Err(AppError::BadRequest("Enter an item name.".into()));
    }
    if form.rate < 1 {
        return Err(AppError::BadRequest("Enter a shelf rate above 0.".into()));
    }
    let kind = ItemKind::parse(&form.item_type)?;
    let barcode = clean_code(&form.barcode);
    let prompt = form.prompt.trim();
    if !form.variants.is_empty() && prompt != "none" {
        return Err(AppError::BadRequest(
            "An item with options uses those rates. Turn asking off, or remove the options.".into(),
        ));
    }
    let saved = if let Some(id) = item_id {
        let updated = sqlx::query(
            r#"
            UPDATE items
            SET section_id = $1, category_id = $2, barcode = $3, name = $4, rate = $5,
                prompt = $6, base_rate = $7, scale = $8,
                item_type = $9, unit_id = COALESCE($10, unit_id)
            WHERE id = $11 AND active
            "#,
        )
        .bind(form.section_id)
        .bind(form.category_id)
        .bind(&barcode)
        .bind(name)
        .bind(form.rate)
        .bind(prompt)
        .bind(form.base_rate)
        .bind(form.scale.max(1))
        .bind(kind.as_str())
        .bind(form.unit_id)
        .bind(id)
        .execute(tx.conn())
        .await
        .map_err(catalog_error)?;
        if updated.rows_affected() == 0 {
            return Err(AppError::NotFound(
                "That item is not in the catalog.".into(),
            ));
        }
        let keep: Vec<i32> = form
            .variants
            .iter()
            .filter_map(|variant| variant.id)
            .collect();
        sqlx::query("DELETE FROM item_variants WHERE item_id = $1 AND NOT (id = ANY($2))")
            .bind(id)
            .bind(&keep)
            .execute(tx.conn())
            .await
            .map_err(catalog_error)?;
        sqlx::query("UPDATE item_variants SET barcode = NULL WHERE item_id = $1")
            .bind(id)
            .execute(tx.conn())
            .await
            .map_err(catalog_error)?;
        id
    } else {
        let sort_order = sqlx::query_scalar::<_, i32>(
            "SELECT COALESCE(MAX(sort_order), 0) + 1 FROM items WHERE section_id = $1",
        )
        .bind(form.section_id)
        .fetch_one(tx.conn())
        .await?;
        sqlx::query_scalar(
            r#"
            INSERT INTO items (
                section_id, category_id, barcode, name, rate, prompt, base_rate, scale,
                sort_order, item_type, unit_id
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            RETURNING id
            "#,
        )
        .bind(form.section_id)
        .bind(form.category_id)
        .bind(&barcode)
        .bind(name)
        .bind(form.rate)
        .bind(prompt)
        .bind(form.base_rate)
        .bind(form.scale.max(1))
        .bind(sort_order)
        .bind(kind.as_str())
        .bind(form.unit_id)
        .fetch_one(tx.conn())
        .await
        .map_err(catalog_error)?
    };

    for (index, variant) in form.variants.iter().enumerate() {
        let variant_id = if let Some(id) = variant.id {
            sqlx::query(
                r#"
                UPDATE item_variants
                SET name = $1, rate = $2, units = $3, barcode = $4, sort_order = $5, unit_id = COALESCE($6, unit_id)
                WHERE id = $7 AND item_id = $8
                "#,
            )
            .bind(variant.name.trim())
            .bind(variant.rate)
            .bind(variant.units.max(1))
            .bind(clean_code(&variant.barcode))
            .bind(index as i32)
            .bind(variant.unit_id)
            .bind(id)
            .bind(saved)
            .execute(tx.conn())
            .await
            .map_err(catalog_error)?;
            id
        } else {
            sqlx::query_scalar(
                r#"
                INSERT INTO item_variants (item_id, name, rate, units, barcode, sort_order, unit_id)
                VALUES ($1, $2, $3, $4, $5, $6, $7)
                RETURNING id
                "#,
            )
            .bind(saved)
            .bind(variant.name.trim())
            .bind(variant.rate)
            .bind(variant.units.max(1))
            .bind(clean_code(&variant.barcode))
            .bind(index as i32)
            .bind(variant.unit_id)
            .fetch_one(tx.conn())
            .await
            .map_err(catalog_error)?
        };
        for attribute in &variant.attributes {
            let attribute_id: i32 = sqlx::query_scalar(
                r#"
                INSERT INTO catalog_attributes (name)
                VALUES ($1)
                ON CONFLICT (tenant_id, name) DO UPDATE SET name = EXCLUDED.name
                RETURNING id
                "#,
            )
            .bind(attribute.name.trim())
            .fetch_one(tx.conn())
            .await?;
            sqlx::query(
                r#"
                INSERT INTO catalog_variant_attributes (variant_id, attribute_id, value)
                VALUES ($1, $2, $3)
                ON CONFLICT (variant_id, attribute_id) DO UPDATE SET value = EXCLUDED.value
                "#,
            )
            .bind(variant_id)
            .bind(attribute_id)
            .bind(attribute.value.trim())
            .execute(tx.conn())
            .await?;
        }
    }
    emit(tx, catalog_contract::events::ITEM_SAVED, saved).await?;
    Ok(saved)
}

pub async fn set_item_prompt(
    tx: &mut TenantTx<'_>,
    item_id: i32,
    request: &ItemPromptRequest,
) -> Result<(), AppError> {
    let updated = sqlx::query(
        "UPDATE items SET prompt = $1, base_rate = $2, scale = $3 WHERE id = $4 AND active",
    )
    .bind(request.prompt.trim())
    .bind(request.base_rate)
    .bind(request.scale.max(1))
    .bind(item_id)
    .execute(tx.conn())
    .await?;
    if updated.rows_affected() == 0 {
        return Err(AppError::NotFound(
            "That item is not in the catalog.".into(),
        ));
    }
    emit(tx, catalog_contract::events::ITEM_SAVED, item_id).await?;
    Ok(())
}

pub async fn set_item_image(
    tx: &mut TenantTx<'_>,
    item_id: i32,
    ext: &str,
) -> Result<(), AppError> {
    if !matches!(ext, "jpg" | "png" | "webp" | "gif") {
        return Err(AppError::BadRequest(
            "Use a JPEG, PNG, WebP, or GIF.".into(),
        ));
    }
    let updated = sqlx::query(
        "UPDATE items SET image_ext = $1, image_rev = image_rev + 1 WHERE id = $2 AND active",
    )
    .bind(ext)
    .bind(item_id)
    .execute(tx.conn())
    .await?;
    if updated.rows_affected() == 0 {
        return Err(AppError::NotFound(
            "That item is no longer in the catalog.".into(),
        ));
    }
    Ok(())
}

pub async fn clear_item_image(tx: &mut TenantTx<'_>, item_id: i32) -> Result<(), AppError> {
    let updated = sqlx::query("UPDATE items SET image_ext = NULL WHERE id = $1 AND active")
        .bind(item_id)
        .execute(tx.conn())
        .await?;
    if updated.rows_affected() == 0 {
        return Err(AppError::NotFound(
            "That item is no longer in the catalog.".into(),
        ));
    }
    Ok(())
}

pub async fn list_units(tx: &mut TenantTx<'_>) -> Result<Vec<UnitDto>, AppError> {
    let rows: Vec<(i32, String, String, String, i32)> = sqlx::query_as(
        "SELECT id, code, name, base_code, factor FROM catalog_units ORDER BY sort_order, id",
    )
    .fetch_all(tx.conn())
    .await?;
    Ok(rows
        .into_iter()
        .map(|(id, code, name, base_code, factor)| UnitDto {
            id,
            code,
            name,
            base_code,
            factor,
        })
        .collect())
}

pub async fn save_unit(tx: &mut TenantTx<'_>, form: &UnitForm) -> Result<i32, AppError> {
    let code = form.code.trim().to_ascii_lowercase();
    if code.is_empty() || form.name.trim().is_empty() {
        return Err(AppError::BadRequest("Enter a unit code and name.".into()));
    }
    if form.factor < 1 {
        return Err(AppError::BadRequest(
            "A unit factor must be at least 1.".into(),
        ));
    }
    sqlx::query_scalar(
        r#"
        INSERT INTO catalog_units (code, name, base_code, factor, sort_order)
        VALUES ($1, $2, $3, $4, (SELECT COALESCE(MAX(sort_order), 0) + 1 FROM catalog_units))
        ON CONFLICT (tenant_id, code) DO UPDATE
        SET name = EXCLUDED.name, base_code = EXCLUDED.base_code, factor = EXCLUDED.factor
        RETURNING id
        "#,
    )
    .bind(&code)
    .bind(form.name.trim())
    .bind(form.base_code.trim())
    .bind(form.factor)
    .fetch_one(tx.conn())
    .await
    .map_err(AppError::from)
}

const ITEM_SQL: &str = r#"
    SELECT i.id, i.name, i.section_id, i.category_id, i.barcode, i.rate, i.active,
           i.item_type, u.code
    FROM items i
    LEFT JOIN catalog_units u ON u.id = i.unit_id
"#;

fn map_item(
    row: (
        i32,
        String,
        i32,
        i32,
        Option<String>,
        i32,
        bool,
        String,
        Option<String>,
    ),
) -> ItemRef {
    ItemRef {
        id: row.0,
        name: row.1,
        section_id: row.2,
        category_id: row.3,
        barcode: row.4,
        rate: row.5,
        active: row.6,
        item_type: row.7,
        unit_code: row.8,
    }
}

pub struct Query;

#[derive(sqlx::FromRow)]
struct SnapshotItemRow {
    id: i32,
    section_id: i32,
    category_id: i32,
    category_name: String,
    barcode: Option<String>,
    name: String,
    rate: i32,
    avg_cost: i64,
    stock: i32,
    portion: String,
    group_name: Option<String>,
    signature: bool,
    prompt: String,
    base_rate: Option<i32>,
    scale: i32,
    image_ext: Option<String>,
    image_rev: i32,
}

#[async_trait::async_trait]
impl catalog_contract::CatalogQuery for Query {
    async fn get_item(&self, tx: &mut TenantTx<'_>, id: i32) -> Result<Option<ItemRef>, AppError> {
        let row = sqlx::query_as(&format!("{ITEM_SQL} WHERE i.id = $1"))
            .bind(id)
            .fetch_optional(tx.conn())
            .await?;
        Ok(row.map(map_item))
    }

    async fn get_variant(
        &self,
        tx: &mut TenantTx<'_>,
        id: i32,
    ) -> Result<Option<VariantRef>, AppError> {
        let row: Option<(i32, i32, String, i32, i32, Option<String>, Option<String>)> =
            sqlx::query_as(
                r#"
            SELECT v.id, v.item_id, v.name, v.rate, v.units, v.barcode, u.code
            FROM item_variants v
            LEFT JOIN catalog_units u ON u.id = v.unit_id
            WHERE v.id = $1
            "#,
            )
            .bind(id)
            .fetch_optional(tx.conn())
            .await?;
        Ok(row.map(
            |(id, item_id, name, rate, units, barcode, unit_code)| VariantRef {
                id,
                item_id,
                name,
                rate,
                units,
                barcode,
                unit_code,
            },
        ))
    }

    async fn items_by_ids(
        &self,
        tx: &mut TenantTx<'_>,
        ids: &[i32],
    ) -> Result<Vec<ItemRef>, AppError> {
        let rows = sqlx::query_as(&format!("{ITEM_SQL} WHERE i.id = ANY($1) ORDER BY i.id"))
            .bind(ids)
            .fetch_all(tx.conn())
            .await?;
        Ok(rows.into_iter().map(map_item).collect())
    }

    async fn find_by_barcode(
        &self,
        tx: &mut TenantTx<'_>,
        barcode: &str,
    ) -> Result<Option<ItemRef>, AppError> {
        let row = sqlx::query_as(&format!("{ITEM_SQL} WHERE i.barcode = $1"))
            .bind(barcode)
            .fetch_optional(tx.conn())
            .await?;
        Ok(row.map(map_item))
    }

    async fn list_sections(&self, tx: &mut TenantTx<'_>) -> Result<Vec<SectionRef>, AppError> {
        let rows: Vec<(i32, String, String)> =
            sqlx::query_as("SELECT id, name, short_name FROM sections ORDER BY pos_order, id")
                .fetch_all(tx.conn())
                .await?;
        Ok(rows
            .into_iter()
            .map(|(id, name, short_name)| SectionRef {
                id,
                name,
                short_name,
            })
            .collect())
    }

    async fn list_categories(&self, tx: &mut TenantTx<'_>) -> Result<Vec<CategoryRef>, AppError> {
        let rows: Vec<(i32, String)> =
            sqlx::query_as("SELECT id, name FROM categories ORDER BY sort_order, id")
                .fetch_all(tx.conn())
                .await?;
        Ok(rows
            .into_iter()
            .map(|(id, name)| CategoryRef { id, name })
            .collect())
    }

    async fn is_stock_tracked(
        &self,
        tx: &mut TenantTx<'_>,
        item_id: i32,
    ) -> Result<bool, AppError> {
        let kind: Option<String> = sqlx::query_scalar("SELECT item_type FROM items WHERE id = $1")
            .bind(item_id)
            .fetch_optional(tx.conn())
            .await?;
        Ok(kind.as_deref().map(tracks_stock).unwrap_or(false))
    }

    async fn set_pos_layout(
        &self,
        tx: &mut TenantTx<'_>,
        layout: &[(i32, bool, i32)],
    ) -> Result<(), AppError> {
        for (section_id, show_on_pos, pos_order) in layout {
            sqlx::query("UPDATE sections SET show_on_pos = $1, pos_order = $2 WHERE id = $3")
                .bind(show_on_pos)
                .bind(pos_order)
                .bind(section_id)
                .execute(tx.conn())
                .await?;
        }
        Ok(())
    }

    async fn snapshot(&self, tx: &mut TenantTx<'_>) -> Result<CatalogSnapshot, AppError> {
        let sections = sqlx::query_as::<_, (i32, String, String, String, String, bool, i32)>(
            "SELECT id, name, short_name, kind, accent, show_on_pos, pos_order FROM sections ORDER BY pos_order, id",
        )
        .fetch_all(tx.conn())
        .await?
        .into_iter()
        .map(|(id, name, short_name, kind, accent, show_on_pos, pos_order)| CatalogSection {
            id,
            name,
            short_name,
            kind,
            accent,
            show_on_pos,
            pos_order,
        })
        .collect();
        let categories = sqlx::query_as::<_, (i32, String, i32)>(
            "SELECT id, name, sort_order FROM categories ORDER BY sort_order, id",
        )
        .fetch_all(tx.conn())
        .await?
        .into_iter()
        .map(|(id, name, sort_order)| CatalogCategory {
            id,
            name,
            sort_order,
        })
        .collect();
        let rows = sqlx::query_as::<_, SnapshotItemRow>(
            r#"
            SELECT i.id, i.section_id, i.category_id, c.name AS category_name, i.barcode, i.name,
                   i.rate, COALESCE(b.avg_cost, 0) AS avg_cost, COALESCE(b.stock, 0) AS stock,
                   i.portion, i.group_name, i.signature,
                   i.prompt, i.base_rate, i.scale, i.image_ext, i.image_rev
            FROM items i
            JOIN categories c ON c.id = i.category_id
            LEFT JOIN inventory_balances b ON b.item_id = i.id
            WHERE i.active
            ORDER BY i.sort_order, i.id
            "#,
        )
        .fetch_all(tx.conn())
        .await?;
        let variants: Vec<CatalogVariant> = sqlx::query_as::<_, (i32, i32, String, i32, i32, Option<String>)>(
            "SELECT id, item_id, name, rate, units, barcode FROM item_variants ORDER BY sort_order, id",
        )
        .fetch_all(tx.conn())
        .await?
        .into_iter()
        .map(|(id, item_id, name, rate, units, barcode)| CatalogVariant {
            id,
            item_id,
            name,
            rate,
            units,
            barcode,
        })
        .collect();
        let items = rows
            .into_iter()
            .map(|row| CatalogItem {
                id: row.id,
                section_id: row.section_id,
                category_id: row.category_id,
                category_name: row.category_name,
                barcode: row.barcode,
                name: row.name,
                rate: row.rate,
                cost: row.avg_cost,
                stock: row.stock,
                portion: row.portion,
                group_name: row.group_name,
                signature: row.signature,
                prompt: row.prompt,
                base_rate: row.base_rate,
                scale: row.scale,
                image_url: item_image_url(row.id, row.image_ext.as_deref(), row.image_rev),
                variants: variants
                    .iter()
                    .filter(|variant| variant.item_id == row.id)
                    .cloned()
                    .collect(),
            })
            .collect();
        Ok(CatalogSnapshot {
            sections,
            categories,
            items,
        })
    }
}

fn item_image_url(id: i32, ext: Option<&str>, rev: i32) -> Option<String> {
    let ext = ext?;
    if !matches!(ext, "jpg" | "png" | "webp" | "gif") {
        return None;
    }
    Some(format!("/api/v1/catalog/items/{id}/image?v={rev}"))
}
