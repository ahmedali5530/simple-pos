use sqlx::PgPool;

use crate::barcode::{codes_match, embedded_sale, parse_embedded, prompt_sale};
use platform_kernel::error::AppError;
use crate::models::{
    BarcodeRule, BillLineInput, Category, ItemForm, ItemPromptRequest, ItemVariant, LockedItem,
    LockedVariant, PosItem, PosSection, PosSnapshot, PosUser, PostedBill, PrinterSettings, SavedItem,
    SectionChoice,
    SectionLayout, ShopConfig,
};

struct Sale {
    item_id: i32,
    variant_id: Option<i32>,
    line_name: String,
    rate: i32,
    qty: i32,
    units: i32,
    stock_units: i32,
    amount: i32,
    cost_total: i64,
}

#[derive(Debug, sqlx::FromRow)]
struct ItemRow {
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

#[derive(sqlx::FromRow)]
struct ShopRow {
    name: String,
    currency: String,
    locale: String,
    bill_prefix: String,
}

#[derive(sqlx::FromRow)]
struct PrinterRow {
    print_enabled: bool,
    printer_kind: String,
    printer_target: String,
    printer_port: i32,
}

#[derive(sqlx::FromRow)]
struct RuleRow {
    prefix: String,
    mode: String,
    scale: i32,
}

pub async fn current_shop(pool: &PgPool, fallback: &ShopConfig) -> Result<ShopConfig, AppError> {
    let mut tx = crate::tenant::begin(pool).await?;
    let result = current_shop_tx(&mut tx, fallback).await?;
    tx.commit().await?;
    Ok(result)
}

pub async fn current_shop_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    fallback: &ShopConfig,
) -> Result<ShopConfig, AppError> {
    let row = sqlx::query_as::<_, ShopRow>(
        "SELECT name, currency, locale, bill_prefix FROM shop_settings WHERE id = 1",
    )
    .fetch_optional(&mut **tx)
    .await?;
    let Some(row) = row else {
        return Ok(fallback.clone());
    };
    let rules = sqlx::query_as::<_, RuleRow>(
        "SELECT prefix, mode, scale FROM barcode_rules ORDER BY sort_order, id",
    )
    .fetch_all(&mut **tx)
    .await?;
    Ok(ShopConfig {
        name: row.name,
        currency: row.currency,
        locale: row.locale,
        bill_prefix: row.bill_prefix,
        barcode_rules: rules
            .into_iter()
            .map(|rule| BarcodeRule {
                prefix: rule.prefix,
                mode: rule.mode,
                scale: rule.scale,
            })
            .collect(),
    })
}

pub async fn load_pos(pool: &PgPool, shop: &ShopConfig) -> Result<PosSnapshot, AppError> {
    let mut tx = crate::tenant::begin(pool).await?;
    let result = load_pos_tx(&mut tx, shop).await?;
    tx.commit().await?;
    Ok(result)
}

pub async fn load_pos_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    shop: &ShopConfig,
) -> Result<PosSnapshot, AppError> {
    let shop = current_shop_tx(tx, shop).await?;
    let shop = &shop;
    let all_sections = sqlx::query_as::<_, SectionChoice>(
        r#"
        SELECT id, name, short_name, kind, accent, show_on_pos, pos_order
        FROM sections
        ORDER BY pos_order, id
        "#,
    )
    .fetch_all(&mut **tx)
    .await?;

    let categories = sqlx::query_as::<_, Category>(
        r#"
        SELECT id, name, sort_order
        FROM categories
        ORDER BY sort_order, id
        "#,
    )
    .fetch_all(&mut **tx)
    .await?;

    let rows = sqlx::query_as::<_, ItemRow>(
        r#"
        SELECT
            i.id,
            i.section_id,
            i.category_id,
            c.name AS category_name,
            i.barcode,
            i.name,
            i.rate,
            i.avg_cost,
            i.stock,
            i.portion,
            i.group_name,
            i.signature,
            i.prompt,
            i.base_rate,
            i.scale,
            i.image_ext,
            i.image_rev
        FROM items i
        JOIN categories c ON c.id = i.category_id
        JOIN sections s ON s.id = i.section_id
        WHERE i.active
        ORDER BY i.sort_order, i.id
        "#,
    )
    .fetch_all(&mut **tx)
    .await?;

    let item_ids: Vec<i32> = rows.iter().map(|item| item.id).collect();
    let variants = if item_ids.is_empty() {
        Vec::new()
    } else {
        sqlx::query_as::<_, ItemVariant>(
            r#"
            SELECT id, item_id, name, rate, units, barcode
            FROM item_variants
            WHERE item_id = ANY($1)
            ORDER BY sort_order, id
            "#,
        )
        .bind(&item_ids)
        .fetch_all(&mut **tx)
        .await?
    };

    let items: Vec<PosItem> = rows
        .into_iter()
        .map(|row| PosItem {
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

    let users = sqlx::query_as::<_, PosUser>(
        "SELECT id, name, role, active FROM users ORDER BY lower(name), id",
    )
    .fetch_all(&mut **tx)
    .await?;

    let last_section_id = sqlx::query_scalar::<_, Option<i32>>(
        "SELECT last_section_id FROM pos_state WHERE id = 1",
    )
    .fetch_optional(&mut **tx)
    .await?
    .flatten();

    let printer = sqlx::query_as::<_, PrinterRow>(
        r#"
        SELECT print_enabled, printer_kind, printer_target, printer_port
        FROM shop_settings
        WHERE id = 1
        "#,
    )
    .fetch_optional(&mut **tx)
    .await?
    .map(|row| PrinterSettings {
        enabled: row.print_enabled,
        kind: row.printer_kind,
        target: row.printer_target,
        port: row.printer_port,
    })
    .unwrap_or(PrinterSettings {
        enabled: false,
        kind: "network".into(),
        target: String::new(),
        port: 9100,
    });

    let sections = all_sections
        .iter()
        .map(|section| PosSection {
            id: section.id,
            name: section.name.clone(),
            short_name: section.short_name.clone(),
            kind: section.kind.clone(),
            accent: section.accent.clone(),
            items: items
                .iter()
                .filter(|item| item.section_id == section.id)
                .cloned()
                .collect(),
        })
        .collect();

    Ok(PosSnapshot {
        shop_name: shop.name.clone(),
        bill_prefix: shop.bill_prefix.clone(),
        currency: shop.currency.clone(),
        locale: shop.locale.clone(),
        barcode_rules: shop.barcode_rules.clone(),
        printer,
        sections,
        all_sections,
        categories,
        users,
        last_section_id,
    })
}

pub async fn set_pos_focus(pool: &PgPool, section_id: i32) -> Result<i32, AppError> {
    let mut tx = crate::tenant::begin(pool).await?;
    let saved = set_pos_focus_tx(&mut tx, section_id).await?;
    tx.commit().await?;
    Ok(saved)
}

pub async fn set_pos_focus_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    section_id: i32,
) -> Result<i32, AppError> {
    let updated = sqlx::query(
        r#"
        UPDATE pos_state
        SET last_section_id = $1
        WHERE id = 1
          AND EXISTS (SELECT 1 FROM sections WHERE id = $1)
        "#,
    )
    .bind(section_id)
    .execute(&mut **tx)
    .await?;
    if updated.rows_affected() == 0 {
        return Err(AppError::NotFound(
            "That section is no longer in the catalog.".into(),
        ));
    }
    Ok(section_id)
}

pub async fn set_pos_sections(
    pool: &PgPool,
    shop: &ShopConfig,
    layout: &[SectionLayout],
) -> Result<PosSnapshot, AppError> {
    let mut tx = crate::tenant::begin(pool).await?;
    let pos = set_pos_sections_tx(&mut tx, shop, layout).await?;
    tx.commit().await?;
    Ok(pos)
}

pub async fn set_pos_sections_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    shop: &ShopConfig,
    layout: &[SectionLayout],
) -> Result<PosSnapshot, AppError> {
    if layout.is_empty() {
        return Err(AppError::BadRequest(
            "Choose at least one section for the till.".into(),
        ));
    }
    if !layout.iter().any(|section| section.show_on_pos) {
        return Err(AppError::BadRequest(
            "Keep at least one section on the till.".into(),
        ));
    }
    let mut seen = std::collections::HashSet::new();
    for section in layout {
        if !seen.insert(section.id) {
            return Err(AppError::BadRequest(
                "Each section can only appear once.".into(),
            ));
        }
    }
    let known = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM sections WHERE id = ANY($1)")
        .bind(
            layout
                .iter()
                .map(|section| section.id)
                .collect::<Vec<_>>(),
        )
        .fetch_one(&mut **tx)
        .await?;
    if known != layout.len() as i64 {
        return Err(AppError::NotFound(
            "One of those sections is no longer in the catalog.".into(),
        ));
    }

    for (index, section) in layout.iter().enumerate() {
        sqlx::query(
            "UPDATE sections SET show_on_pos = $1, pos_order = $2 WHERE id = $3",
        )
        .bind(section.show_on_pos)
        .bind(index as i32)
        .bind(section.id)
        .execute(&mut **tx)
        .await?;
    }
    load_pos_tx(tx, shop).await
}

pub async fn set_item_prompt(
    pool: &PgPool,
    shop: &ShopConfig,
    item_id: i32,
    request: &ItemPromptRequest,
) -> Result<PosSnapshot, AppError> {
    let prompt = request.prompt.trim();
    if prompt != "none" && prompt != "quantity" && prompt != "price" {
        return Err(AppError::BadRequest(
            "Choose off, quantity or weight, or price.".into(),
        ));
    }
    let (base_rate, scale) = if prompt == "none" {
        (None, 1)
    } else {
        let base_rate = request.base_rate.unwrap_or(0);
        if base_rate < 1 {
            return Err(AppError::BadRequest("Set a base rate above 0.".into()));
        }
        if request.scale < 1 {
            return Err(AppError::BadRequest("Stock per 1 must be at least 1.".into()));
        }
        (Some(base_rate), request.scale)
    };
    let mut tx = crate::tenant::begin(pool).await?;
    let variants = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM item_variants WHERE item_id = $1")
        .bind(item_id)
        .fetch_one(&mut *tx)
        .await?;
    if variants > 0 && prompt != "none" {
        return Err(AppError::BadRequest(
            "An item with options uses those rates. Turn asking off, or remove the options.".into(),
        ));
    }
    let updated = sqlx::query(
        r#"
        UPDATE items
        SET prompt = $1, base_rate = $2, scale = $3
        WHERE id = $4 AND active
        "#,
    )
    .bind(prompt)
    .bind(base_rate)
    .bind(scale)
    .bind(item_id)
    .execute(&mut *tx)
    .await?;
    if updated.rows_affected() == 0 {
        return Err(AppError::NotFound("That item is not in the catalog.".into()));
    }
    let pos = load_pos_tx(&mut tx, shop).await?;
    tx.commit().await?;
    Ok(pos)
}

fn clean_code(value: &Option<String>) -> Option<String> {
    value
        .as_ref()
        .map(|code| code.trim().to_string())
        .filter(|code| !code.is_empty())
}

fn map_catalog_error(err: sqlx::Error) -> AppError {
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
        _ => AppError::Db(err),
    }
}

pub async fn save_item(
    pool: &PgPool,
    shop: &ShopConfig,
    item_id: Option<i32>,
    request: &ItemForm,
) -> Result<SavedItem, AppError> {
    let name = request.name.trim();
    if name.is_empty() {
        return Err(AppError::BadRequest("Enter an item name.".into()));
    }
    if request.rate < 1 {
        return Err(AppError::BadRequest("Enter a shelf rate above 0.".into()));
    }
    if request.cost < 0 || request.cost > 10_000_000_000 {
        return Err(AppError::BadRequest(
            "Enter a cost of 0 or more.".into(),
        ));
    }
    let barcode = clean_code(&request.barcode);
    let prompt = request.prompt.trim();
    if prompt != "none" && prompt != "quantity" && prompt != "price" {
        return Err(AppError::BadRequest(
            "Choose off, quantity or weight, or price.".into(),
        ));
    }
    if !request.variants.is_empty() && prompt != "none" {
        return Err(AppError::BadRequest(
            "An item with options uses those rates. Turn asking off, or remove the options.".into(),
        ));
    }
    let (base_rate, scale) = if prompt == "none" {
        (None, 1)
    } else {
        let base_rate = request.base_rate.unwrap_or(0);
        if base_rate < 1 {
            return Err(AppError::BadRequest("Set a base rate above 0.".into()));
        }
        if request.scale < 1 {
            return Err(AppError::BadRequest("Stock per 1 must be at least 1.".into()));
        }
        (Some(base_rate), request.scale)
    };

    let mut names = std::collections::HashSet::new();
    let mut codes = std::collections::HashSet::new();
    if let Some(code) = &barcode {
        codes.insert(code.clone());
    }
    let mut options = Vec::new();
    for (index, variant) in request.variants.iter().enumerate() {
        let variant_name = variant.name.trim();
        if variant_name.is_empty() {
            return Err(AppError::BadRequest("Name each option.".into()));
        }
        if !names.insert(variant_name.to_lowercase()) {
            return Err(AppError::BadRequest("Each option needs its own name.".into()));
        }
        if variant.rate < 1 {
            return Err(AppError::BadRequest(
                format!("{variant_name} needs a rate above 0."),
            ));
        }
        if variant.units < 1 {
            return Err(AppError::BadRequest(
                format!("{variant_name} must take at least 1 from stock."),
            ));
        }
        let variant_barcode = clean_code(&variant.barcode);
        if let Some(code) = &variant_barcode {
            if !codes.insert(code.clone()) {
                return Err(AppError::BadRequest(
                    "That barcode is used more than once on this item.".into(),
                ));
            }
        }
        options.push((index as i32, variant.id, variant_name.to_string(), variant.rate, variant.units, variant_barcode));
    }

    let mut tx = crate::tenant::begin(pool).await?;
    let section_ok = sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM sections WHERE id = $1)")
        .bind(request.section_id)
        .fetch_one(&mut *tx)
        .await?;
    if !section_ok {
        return Err(AppError::NotFound("That section is no longer in the catalog.".into()));
    }
    let category_ok = sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM categories WHERE id = $1)")
        .bind(request.category_id)
        .fetch_one(&mut *tx)
        .await?;
    if !category_ok {
        return Err(AppError::NotFound("That category is no longer in the catalog.".into()));
    }
    let saved_id = if let Some(id) = item_id {
        let updated = sqlx::query(
            r#"
            UPDATE items
            SET section_id = $1, category_id = $2, barcode = NULL, name = $3, rate = $4,
                prompt = $5, base_rate = $6, scale = $7, avg_cost = $8
            WHERE id = $9 AND active
            "#,
        )
        .bind(request.section_id)
        .bind(request.category_id)
        .bind(name)
        .bind(request.rate)
        .bind(prompt)
        .bind(base_rate)
        .bind(scale)
        .bind(request.cost)
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(map_catalog_error)?;
        if updated.rows_affected() == 0 {
            return Err(AppError::NotFound("That item is not in the catalog.".into()));
        }
        let keep: Vec<i32> = options.iter().filter_map(|option| option.1).collect();
        sqlx::query("DELETE FROM item_variants WHERE item_id = $1 AND NOT (id = ANY($2))")
            .bind(id)
            .bind(&keep)
            .execute(&mut *tx)
            .await
            .map_err(map_catalog_error)?;
        sqlx::query("UPDATE item_variants SET barcode = NULL WHERE item_id = $1")
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(map_catalog_error)?;
        sqlx::query("UPDATE items SET barcode = $1 WHERE id = $2")
            .bind(&barcode)
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(map_catalog_error)?;
        for (sort_order, variant_id, variant_name, rate, units, variant_barcode) in &options {
            if let Some(variant_id) = variant_id {
                let changed = sqlx::query(
                    r#"
                    UPDATE item_variants
                    SET name = $1, rate = $2, units = $3, barcode = $4, sort_order = $5
                    WHERE id = $6 AND item_id = $7
                    "#,
                )
                .bind(variant_name)
                .bind(rate)
                .bind(units)
                .bind(variant_barcode)
                .bind(sort_order)
                .bind(variant_id)
                .bind(id)
                .execute(&mut *tx)
                .await
                .map_err(map_catalog_error)?;
                if changed.rows_affected() == 0 {
                    return Err(AppError::BadRequest(
                        "One of those options is not on this item.".into(),
                    ));
                }
            } else {
                sqlx::query(
                    r#"
                    INSERT INTO item_variants (item_id, name, rate, units, barcode, sort_order)
                    VALUES ($1, $2, $3, $4, $5, $6)
                    "#,
                )
                .bind(id)
                .bind(variant_name)
                .bind(rate)
                .bind(units)
                .bind(variant_barcode)
                .bind(sort_order)
                .execute(&mut *tx)
                .await
                .map_err(map_catalog_error)?;
            }
        }
        id
    } else {
        let sort_order = sqlx::query_scalar::<_, i32>(
            "SELECT COALESCE(MAX(sort_order), 0) + 1 FROM items WHERE section_id = $1",
        )
        .bind(request.section_id)
        .fetch_one(&mut *tx)
        .await?;
        let id = sqlx::query_scalar::<_, i32>(
            r#"
            INSERT INTO items (
                section_id, category_id, barcode, name, rate, stock, prompt, base_rate, scale, avg_cost, sort_order
            )
            VALUES ($1, $2, $3, $4, $5, 0, $6, $7, $8, $9, $10)
            RETURNING id
            "#,
        )
        .bind(request.section_id)
        .bind(request.category_id)
        .bind(&barcode)
        .bind(name)
        .bind(request.rate)
        .bind(prompt)
        .bind(base_rate)
        .bind(scale)
        .bind(request.cost)
        .bind(sort_order)
        .fetch_one(&mut *tx)
        .await
        .map_err(map_catalog_error)?;
        for (sort_order, _, variant_name, rate, units, variant_barcode) in &options {
            sqlx::query(
                r#"
                INSERT INTO item_variants (item_id, name, rate, units, barcode, sort_order)
                VALUES ($1, $2, $3, $4, $5, $6)
                "#,
            )
            .bind(id)
            .bind(variant_name)
            .bind(rate)
            .bind(units)
            .bind(variant_barcode)
            .bind(sort_order)
            .execute(&mut *tx)
            .await
            .map_err(map_catalog_error)?;
        }
        id
    };
    let pos = load_pos_tx(&mut tx, shop).await?;
    tx.commit().await?;
    Ok(SavedItem {
        item_id: saved_id,
        pos,
    })
}

fn item_image_url(id: i32, ext: Option<&str>, rev: i32) -> Option<String> {
    let ext = ext?;
    if !matches!(ext, "jpg" | "png" | "webp" | "gif") {
        return None;
    }
    Some(format!("/api/v1/catalog/items/{id}/image?v={rev}"))
}

pub async fn item_image_ext(pool: &PgPool, item_id: i32) -> Result<Option<String>, AppError> {
    let mut tx = crate::tenant::begin(pool).await?;
    let row = sqlx::query_scalar::<_, Option<String>>(
        "SELECT image_ext FROM items WHERE id = $1 AND active",
    )
    .bind(item_id)
    .fetch_optional(&mut *tx)
    .await?;
    tx.commit().await?;
    match row {
        None => Err(AppError::NotFound(
            "That item is no longer in the catalog.".into(),
        )),
        Some(ext) => Ok(ext),
    }
}

pub async fn set_item_image(
    pool: &PgPool,
    shop: &ShopConfig,
    item_id: i32,
    ext: &str,
) -> Result<SavedItem, AppError> {
    if !matches!(ext, "jpg" | "png" | "webp" | "gif") {
        return Err(AppError::BadRequest(
            "Use a JPEG, PNG, WebP, or GIF.".into(),
        ));
    }
    let mut tx = crate::tenant::begin(pool).await?;
    let updated = sqlx::query(
        r#"
        UPDATE items
        SET image_ext = $1, image_rev = image_rev + 1
        WHERE id = $2 AND active
        "#,
    )
    .bind(ext)
    .bind(item_id)
    .execute(&mut *tx)
    .await?;
    if updated.rows_affected() == 0 {
        return Err(AppError::NotFound(
            "That item is no longer in the catalog.".into(),
        ));
    }
    let pos = load_pos_tx(&mut tx, shop).await?;
    tx.commit().await?;
    Ok(SavedItem {
        item_id,
        pos,
    })
}

pub async fn clear_item_image(
    pool: &PgPool,
    shop: &ShopConfig,
    item_id: i32,
) -> Result<SavedItem, AppError> {
    let mut tx = crate::tenant::begin(pool).await?;
    let exists = sqlx::query_scalar::<_, i32>("SELECT id FROM items WHERE id = $1 AND active")
        .bind(item_id)
        .fetch_optional(&mut *tx)
        .await?;
    if exists.is_none() {
        return Err(AppError::NotFound(
            "That item is no longer in the catalog.".into(),
        ));
    }
    sqlx::query("UPDATE items SET image_ext = NULL WHERE id = $1")
        .bind(item_id)
        .execute(&mut *tx)
        .await?;
    let pos = load_pos_tx(&mut tx, shop).await?;
    tx.commit().await?;
    Ok(SavedItem {
        item_id,
        pos,
    })
}

pub async fn post_bill(
    pool: &PgPool,
    shop: &ShopConfig,
    user_id: i32,
    section_id: i32,
    lines: &[BillLineInput],
    customer: Option<&str>,
) -> Result<PostedBill, AppError> {
    let mut tx = crate::tenant::begin(pool).await?;
    let posted = post_bill_tx(&mut tx, shop, user_id, section_id, lines, customer).await?;
    tx.commit().await?;
    Ok(posted)
}

pub async fn post_bill_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    shop: &ShopConfig,
    user_id: i32,
    section_id: i32,
    lines: &[BillLineInput],
    customer: Option<&str>,
) -> Result<PostedBill, AppError> {
    if lines.is_empty() {
        return Err(AppError::BadRequest(
            "Add at least one item before posting the bill.".into(),
        ));
    }
    let customer = customer.map(str::trim).filter(|name| !name.is_empty());
    if customer.is_some_and(|name| name.chars().count() > 80) {
        return Err(AppError::BadRequest(
            "Keep the customer name to 80 characters.".into(),
        ));
    }
    let shop = current_shop_tx(tx, shop).await?;
    let shop = &shop;
    let section_name = sqlx::query_scalar::<_, String>(
        "SELECT name FROM sections WHERE id = $1",
    )
    .bind(section_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| AppError::NotFound("That section is not in the catalog.".into()))?;

    let mut total = 0_i32;
    let mut prepared: Vec<Sale> = Vec::with_capacity(lines.len());
    let mut taken: std::collections::HashMap<i32, i32> = std::collections::HashMap::new();
    let mut locked: std::collections::HashMap<i32, LockedItem> = std::collections::HashMap::new();

    for line in lines {
        if line.qty < 1 {
            return Err(AppError::BadRequest(
                "Quantity has to be at least 1.".into(),
            ));
        }
        if line.rate < 1 {
            return Err(AppError::BadRequest("Rate has to be at least 1.".into()));
        }
        let item = if let Some(item) = locked.get(&line.item_id) {
            item.clone()
        } else {
            let item = sqlx::query_as::<_, LockedItem>(
                r#"
                SELECT
                    id,
                    name,
                    barcode,
                    stock,
                    rate,
                    avg_cost,
                    (SELECT COUNT(*) FROM item_variants WHERE item_id = items.id)::int AS variant_count,
                    prompt,
                    base_rate,
                    scale
                FROM items
                WHERE id = $1 AND section_id = $2 AND active
                FOR UPDATE
                "#,
            )
            .bind(line.item_id)
            .bind(section_id)
            .fetch_optional(&mut **tx)
            .await?
            .ok_or_else(|| {
                AppError::BadRequest(format!("That item is not sold in {section_name}."))
            })?;
            locked.insert(item.id, item.clone());
            item
        };

        let (variant_id, units, line_name, catalog_rate, stored_barcode) = match line.variant_id {
            Some(variant_id) => {
                let variant = sqlx::query_as::<_, LockedVariant>(
                    r#"
                    SELECT id, name, rate, units, barcode
                    FROM item_variants
                    WHERE id = $1 AND item_id = $2
                    FOR UPDATE
                    "#,
                )
                .bind(variant_id)
                .bind(item.id)
                .fetch_optional(&mut **tx)
                .await?
                .ok_or_else(|| {
                    AppError::BadRequest(format!("{} does not have that option.", item.name))
                })?;
                (
                    Some(variant.id),
                    variant.units,
                    format!("{} · {}", item.name, variant.name),
                    variant.rate,
                    variant.barcode.or(item.barcode.clone()),
                )
            }
            None => {
                if item.variant_count > 0 {
                    return Err(AppError::BadRequest(format!(
                        "Choose an option for {}.",
                        item.name
                    )));
                }
                (None, 1, item.name.clone(), item.rate, item.barcode.clone())
            }
        };
        let (rate, units, line_name) = if let Some(code) = line.code.as_deref().filter(|code| !code.is_empty()) {
            let embedded = parse_embedded(code, &shop.barcode_rules).ok_or_else(|| {
                AppError::BadRequest("That is not a weight or price barcode.".into())
            })?;
            let matched = stored_barcode
                .as_deref()
                .is_some_and(|barcode| codes_match(barcode, &embedded.item_code));
            if !matched {
                return Err(AppError::BadRequest(
                    "That barcode does not belong to this item.".into(),
                ));
            }
            let (rate, stock_each, label) = embedded_sale(catalog_rate, &embedded).map_err(AppError::BadRequest)?;
            let units = stock_each.unwrap_or(units);
            let line_name = if label.is_empty() {
                line_name
            } else {
                format!("{line_name} · {label}")
            };
            (rate, units, line_name)
        } else if item.prompt != "none" && variant_id.is_none() {
            let base_rate = item.base_rate.ok_or_else(|| {
                AppError::BadRequest(format!("Set a base rate for {}.", item.name))
            })?;
            let entry = line
                .entry
                .as_deref()
                .map(str::trim)
                .filter(|entry| !entry.is_empty())
                .ok_or_else(|| AppError::BadRequest(format!("{} needs a value.", item.name)))?;
            let quote = prompt_sale(&item.prompt, base_rate, item.scale, entry).map_err(AppError::BadRequest)?;
            let line_name = if quote.label.is_empty() {
                line_name
            } else {
                format!("{line_name} · {}", quote.label)
            };
            (quote.rate, quote.units, line_name)
        } else {
            (catalog_rate, units, line_name)
        };
        if line.rate != rate {
            return Err(AppError::BadRequest(format!(
                "The rate for {line_name} is {rate}."
            )));
        }
        let stock_units = units.checked_mul(line.qty).ok_or_else(|| {
            AppError::BadRequest("That line takes too much stock.".into())
        })?;
        let used = taken.entry(item.id).or_insert(0);
        *used = used.checked_add(stock_units).ok_or_else(|| {
            AppError::BadRequest("That bill takes too much stock.".into())
        })?;
        if *used > item.stock {
            return Err(AppError::BadRequest(format!(
                "Only {} left for {}.",
                item.stock, item.name
            )));
        }
        let amount = line.rate.checked_mul(line.qty).ok_or_else(|| {
            AppError::BadRequest("That line amount is too large.".into())
        })?;
        let cost_total = i64::from(stock_units).checked_mul(item.avg_cost).ok_or_else(|| {
            AppError::BadRequest("That line cost is too large.".into())
        })?;
        total = total.checked_add(amount).ok_or_else(|| {
            AppError::BadRequest("That bill total is too large.".into())
        })?;
        prepared.push(Sale {
            item_id: item.id,
            variant_id,
            line_name,
            rate: line.rate,
            qty: line.qty,
            units,
            stock_units,
            amount,
            cost_total,
        });
    }

    let bill_id = sqlx::query_scalar::<_, i32>("SELECT nextval('bills_id_seq')::int")
        .fetch_one(&mut **tx)
        .await?;
    let bill_no = format!("{}-{}", shop.bill_prefix, 1000 + bill_id);
    sqlx::query(
        r#"
        INSERT INTO bills (id, bill_no, section_id, user_id, total, customer)
        VALUES ($1, $2, $3, $4, $5, $6)
        "#,
    )
    .bind(bill_id)
    .bind(&bill_no)
    .bind(section_id)
    .bind(user_id)
    .bind(total)
    .bind(customer)
    .execute(&mut **tx)
    .await?;

    for sale in &prepared {
        sqlx::query(
            r#"
            INSERT INTO bill_lines (bill_id, item_id, variant_id, name, rate, qty, units, amount, cost_total)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            "#,
        )
        .bind(bill_id)
        .bind(sale.item_id)
        .bind(sale.variant_id)
        .bind(&sale.line_name)
        .bind(sale.rate)
        .bind(sale.qty)
        .bind(sale.units)
        .bind(sale.amount)
        .bind(sale.cost_total)
        .execute(&mut **tx)
        .await?;
    }

    let pos = load_pos_tx(tx, shop).await?;
    Ok(PostedBill {
        bill_no,
        total,
        pos,
    })
}
