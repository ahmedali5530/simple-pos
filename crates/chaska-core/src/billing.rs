use sqlx::PgPool;

use crate::barcode::{codes_match, embedded_sale, parse_embedded, prompt_sale};
use crate::error::AppError;
use crate::models::{
    BillLineInput, Category, ItemPromptRequest, ItemVariant, LockedItem, LockedVariant, PosItem,
    PosSection, PosSnapshot, PostedBill, SectionChoice, SectionLayout, ShopConfig,
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
    stock: i32,
    portion: String,
    group_name: Option<String>,
    signature: bool,
    prompt: String,
    base_rate: Option<i32>,
    scale: i32,
}

pub async fn connect(database_url: &str) -> Result<PgPool, AppError> {
    let pool = PgPool::connect(database_url).await?;
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .map_err(|err| AppError::BadRequest(err.to_string()))?;
    Ok(pool)
}

pub async fn load_pos(pool: &PgPool, shop: &ShopConfig) -> Result<PosSnapshot, AppError> {
    let all_sections = sqlx::query_as::<_, SectionChoice>(
        r#"
        SELECT id, name, short_name, kind, accent, show_on_pos, pos_order
        FROM sections
        ORDER BY pos_order, id
        "#,
    )
    .fetch_all(pool)
    .await?;

    let categories = sqlx::query_as::<_, Category>(
        r#"
        SELECT id, name, sort_order
        FROM categories
        ORDER BY sort_order, id
        "#,
    )
    .fetch_all(pool)
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
            i.stock,
            i.portion,
            i.group_name,
            i.signature,
            i.prompt,
            i.base_rate,
            i.scale
        FROM items i
        JOIN categories c ON c.id = i.category_id
        JOIN sections s ON s.id = i.section_id
        WHERE i.active
        ORDER BY i.sort_order, i.id
        "#,
    )
    .fetch_all(pool)
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
        .fetch_all(pool)
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
            stock: row.stock,
            portion: row.portion,
            group_name: row.group_name,
            signature: row.signature,
            prompt: row.prompt,
            base_rate: row.base_rate,
            scale: row.scale,
            variants: variants
                .iter()
                .filter(|variant| variant.item_id == row.id)
                .cloned()
                .collect(),
        })
        .collect();

    let last_section_id = sqlx::query_scalar::<_, Option<i32>>(
        "SELECT last_section_id FROM pos_state WHERE id = 1",
    )
    .fetch_optional(pool)
    .await?
    .flatten();

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
        currency: shop.currency.clone(),
        locale: shop.locale.clone(),
        barcode_rules: shop.barcode_rules.clone(),
        sections,
        all_sections,
        categories,
        last_section_id,
    })
}

pub async fn set_pos_focus(pool: &PgPool, section_id: i32) -> Result<i32, AppError> {
    let updated = sqlx::query(
        r#"
        UPDATE pos_state
        SET last_section_id = $1
        WHERE id = 1
          AND EXISTS (SELECT 1 FROM sections WHERE id = $1)
        "#,
    )
    .bind(section_id)
    .execute(pool)
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
        .fetch_one(pool)
        .await?;
    if known != layout.len() as i64 {
        return Err(AppError::NotFound(
            "One of those sections is no longer in the catalog.".into(),
        ));
    }

    let mut tx = pool.begin().await?;
    for (index, section) in layout.iter().enumerate() {
        sqlx::query(
            "UPDATE sections SET show_on_pos = $1, pos_order = $2 WHERE id = $3",
        )
        .bind(section.show_on_pos)
        .bind(index as i32)
        .bind(section.id)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    load_pos(pool, shop).await
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
    let variants = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM item_variants WHERE item_id = $1")
        .bind(item_id)
        .fetch_one(pool)
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
    .execute(pool)
    .await?;
    if updated.rows_affected() == 0 {
        return Err(AppError::NotFound("That item is not in the catalog.".into()));
    }
    load_pos(pool, shop).await
}

pub async fn post_bill(
    pool: &PgPool,
    shop: &ShopConfig,
    section_id: i32,
    lines: &[BillLineInput],
) -> Result<PostedBill, AppError> {
    if lines.is_empty() {
        return Err(AppError::BadRequest(
            "Add at least one item before posting the bill.".into(),
        ));
    }

    let mut tx = pool.begin().await?;
    let section_name = sqlx::query_scalar::<_, String>(
        "SELECT name FROM sections WHERE id = $1",
    )
    .bind(section_id)
    .fetch_optional(&mut *tx)
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
            .fetch_optional(&mut *tx)
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
                .fetch_optional(&mut *tx)
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
        });
    }

    let bill_id = sqlx::query_scalar::<_, i32>("SELECT nextval('bills_id_seq')::int")
        .fetch_one(&mut *tx)
        .await?;
    let bill_no = format!("CC-{}", 1000 + bill_id);
    sqlx::query(
        r#"
        INSERT INTO bills (id, bill_no, section_id, total)
        VALUES ($1, $2, $3, $4)
        "#,
    )
    .bind(bill_id)
    .bind(&bill_no)
    .bind(section_id)
    .bind(total)
    .execute(&mut *tx)
    .await?;

    for (item_id, stock_units) in &taken {
        sqlx::query("UPDATE items SET stock = stock - $1 WHERE id = $2")
            .bind(stock_units)
            .bind(item_id)
            .execute(&mut *tx)
            .await?;
    }
    for sale in &prepared {
        sqlx::query(
            r#"
            INSERT INTO bill_lines (bill_id, item_id, variant_id, name, rate, qty, units, amount)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
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
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            r#"
            INSERT INTO stock_movements (item_id, qty, reason, bill_id)
            VALUES ($1, $2, $3, $4)
            "#,
        )
        .bind(sale.item_id)
        .bind(-sale.stock_units)
        .bind(format!("POS bill {bill_no}"))
        .bind(bill_id)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    let pos = load_pos(pool, shop).await?;
    Ok(PostedBill {
        bill_no,
        total,
        pos,
    })
}
