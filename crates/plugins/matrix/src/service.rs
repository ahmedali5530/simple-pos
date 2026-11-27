//! Matrix recipe, SKU generation, and stock adjustments.

use platform_kernel::context::TenantTx;
use platform_kernel::error::AppError;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupValueInput {
    pub value: String,
    #[serde(default)]
    pub sort_order: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupInput {
    pub name: String,
    #[serde(default)]
    pub sort_order: i32,
    #[serde(default)]
    pub values: Vec<GroupValueInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecipeInput {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub barcode_prefix: String,
    #[serde(default)]
    pub groups: Vec<GroupInput>,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkuPatch {
    pub barcode: Option<String>,
    pub rate: Option<i32>,
    pub stock: Option<i32>,
    pub avg_cost: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkuAdjust {
    pub direction: String,
    pub qty: i32,
    #[serde(default)]
    pub update_cost: bool,
    pub total: Option<i64>,
    pub note: Option<String>,
}

#[derive(sqlx::FromRow)]
struct SkuRow {
    variant_id: i32,
    item_id: i32,
    fingerprint: String,
    name: String,
    barcode: Option<String>,
    rate: i32,
    stock: i32,
    avg_cost: i64,
    parent_name: String,
    section_name: String,
    section_id: i32,
    accent: String,
}

pub async fn list_enabled_item_ids(tx: &mut TenantTx<'_>) -> Result<Vec<i32>, AppError> {
    let ids = sqlx::query_scalar::<_, i32>(
        "SELECT item_id FROM matrix_items WHERE enabled = true ORDER BY item_id",
    )
    .fetch_all(tx.conn())
    .await?;
    Ok(ids)
}

pub async fn get_recipe(tx: &mut TenantTx<'_>, item_id: i32) -> Result<Value, AppError> {
    ensure_item_exists(tx, item_id).await?;
    let row = sqlx::query_as::<_, (bool, String)>(
        "SELECT enabled, barcode_prefix FROM matrix_items WHERE item_id = $1",
    )
    .bind(item_id)
    .fetch_optional(tx.conn())
    .await?;

    let (enabled, barcode_prefix) = row.unwrap_or((false, String::new()));
    let groups = load_groups(tx, item_id).await?;
    let skus = load_skus(tx, Some(item_id)).await?;
    Ok(json!({
        "item_id": item_id,
        "enabled": enabled,
        "barcode_prefix": barcode_prefix,
        "groups": groups,
        "skus": skus.into_iter().map(sku_json).collect::<Vec<_>>(),
    }))
}

pub async fn save_recipe(
    tx: &mut TenantTx<'_>,
    item_id: i32,
    body: &RecipeInput,
) -> Result<Value, AppError> {
    ensure_item_exists(tx, item_id).await?;
    let prefix = body.barcode_prefix.trim().to_string();
    if prefix.chars().count() > 32 {
        return Err(AppError::BadRequest(
            "Keep the barcode prefix to 32 characters.".into(),
        ));
    }
    validate_groups(&body.groups)?;

    sqlx::query(
        r#"
        INSERT INTO matrix_items (item_id, enabled, barcode_prefix, updated_at)
        VALUES ($1, $2, $3, now())
        ON CONFLICT (tenant_id, item_id) DO UPDATE
          SET enabled = EXCLUDED.enabled,
              barcode_prefix = EXCLUDED.barcode_prefix,
              updated_at = now()
        "#,
    )
    .bind(item_id)
    .bind(body.enabled)
    .bind(&prefix)
    .execute(tx.conn())
    .await?;

    sqlx::query("DELETE FROM matrix_groups WHERE item_id = $1")
        .bind(item_id)
        .execute(tx.conn())
        .await?;

    for (group_index, group) in body.groups.iter().enumerate() {
        let group_id: i32 = sqlx::query_scalar(
            r#"
            INSERT INTO matrix_groups (item_id, name, sort_order)
            VALUES ($1, $2, $3)
            RETURNING id
            "#,
        )
        .bind(item_id)
        .bind(group.name.trim())
        .bind(if group.sort_order != 0 {
            group.sort_order
        } else {
            group_index as i32
        })
        .fetch_one(tx.conn())
        .await?;
        for (value_index, value) in group.values.iter().enumerate() {
            sqlx::query(
                r#"
                INSERT INTO matrix_group_values (group_id, value, sort_order)
                VALUES ($1, $2, $3)
                "#,
            )
            .bind(group_id)
            .bind(value.value.trim())
            .bind(if value.sort_order != 0 {
                value.sort_order
            } else {
                value_index as i32
            })
            .execute(tx.conn())
            .await?;
        }
    }

    get_recipe(tx, item_id).await
}

pub async fn clear_recipe(tx: &mut TenantTx<'_>, item_id: i32) -> Result<(), AppError> {
    sqlx::query("DELETE FROM matrix_items WHERE item_id = $1")
        .bind(item_id)
        .execute(tx.conn())
        .await?;
    Ok(())
}

pub async fn generate(tx: &mut TenantTx<'_>, item_id: i32) -> Result<Value, AppError> {
    let recipe = get_recipe(tx, item_id).await?;
    let enabled = recipe
        .get("enabled")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    if !enabled {
        return Err(AppError::BadRequest(
            "Enable the matrix on this item before generating variants.".into(),
        ));
    }
    let groups = load_group_rows(tx, item_id).await?;
    if groups.is_empty() {
        return Err(AppError::BadRequest(
            "Add at least one variant group with values.".into(),
        ));
    }
    for group in &groups {
        if group.values.is_empty() {
            return Err(AppError::BadRequest(format!(
                "Add values for the {} group.",
                group.name
            )));
        }
    }

    let parent = sqlx::query_as::<_, (String, i32, i64)>(
        r#"
        SELECT i.name, i.rate, COALESCE(b.avg_cost, 0)
        FROM items i
        LEFT JOIN inventory_balances b ON b.item_id = i.id
        WHERE i.id = $1
        "#,
    )
    .bind(item_id)
    .fetch_optional(tx.conn())
    .await?
    .ok_or_else(|| AppError::NotFound("That item is no longer in the catalog.".into()))?;
    let (_parent_name, parent_rate, parent_cost) = parent;

    let prefix: String = sqlx::query_scalar(
        "SELECT barcode_prefix FROM matrix_items WHERE item_id = $1",
    )
    .bind(item_id)
    .fetch_one(tx.conn())
    .await?;

    let combos = cartesian(&groups);
    if combos.len() > 2_000 {
        return Err(AppError::BadRequest(
            "That matrix would create too many variants. Narrow the groups.".into(),
        ));
    }

    let existing = sqlx::query_as::<_, (i32, String, Option<String>, i32, i32, i64)>(
        r#"
        SELECT s.variant_id, s.fingerprint, v.barcode, v.rate, s.stock, s.avg_cost
        FROM matrix_skus s
        JOIN item_variants v ON v.id = s.variant_id
        WHERE s.item_id = $1
        "#,
    )
    .bind(item_id)
    .fetch_all(tx.conn())
    .await?;
    let mut by_fp: std::collections::HashMap<String, (i32, Option<String>, i32, i32, i64)> =
        existing
            .into_iter()
            .map(|(variant_id, fp, barcode, rate, stock, avg_cost)| {
                (fp, (variant_id, barcode, rate, stock, avg_cost))
            })
            .collect();

    let mut next_seq = next_barcode_seq(tx, &prefix).await?;
    let mut keep_variants = Vec::new();

    for (index, combo) in combos.iter().enumerate() {
        let fingerprint = fingerprint_for(combo);
        let name = combo
            .iter()
            .map(|(_, value)| value.as_str())
            .collect::<Vec<_>>()
            .join(" / ");

        let (variant_id, barcode, rate, stock, avg_cost) =
            if let Some(existing) = by_fp.remove(&fingerprint) {
                existing
            } else {
                let barcode = allocate_barcode(tx, &prefix, &mut next_seq).await?;
                (0, Some(barcode), parent_rate, 0, parent_cost)
            };

        let variant_id = if variant_id > 0 {
            sqlx::query(
                r#"
                UPDATE item_variants
                SET name = $2, rate = $3, units = 1, barcode = $4, sort_order = $5
                WHERE id = $1 AND item_id = $6
                "#,
            )
            .bind(variant_id)
            .bind(&name)
            .bind(rate)
            .bind(barcode.as_deref())
            .bind(index as i32)
            .bind(item_id)
            .execute(tx.conn())
            .await?;
            variant_id
        } else {
            sqlx::query_scalar(
                r#"
                INSERT INTO item_variants (item_id, name, rate, units, barcode, sort_order)
                VALUES ($1, $2, $3, 1, $4, $5)
                RETURNING id
                "#,
            )
            .bind(item_id)
            .bind(&name)
            .bind(rate)
            .bind(barcode.as_deref())
            .bind(index as i32)
            .fetch_one(tx.conn())
            .await
            .map_err(barcode_error)?
        };

        sqlx::query("DELETE FROM catalog_variant_attributes WHERE variant_id = $1")
            .bind(variant_id)
            .execute(tx.conn())
            .await?;
        for (group_name, value) in combo {
            let attribute_id: i32 = sqlx::query_scalar(
                r#"
                INSERT INTO catalog_attributes (name)
                VALUES ($1)
                ON CONFLICT (tenant_id, name) DO UPDATE SET name = EXCLUDED.name
                RETURNING id
                "#,
            )
            .bind(group_name)
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
            .bind(value)
            .execute(tx.conn())
            .await?;
        }

        sqlx::query(
            r#"
            INSERT INTO matrix_skus (variant_id, item_id, fingerprint, stock, avg_cost, updated_at)
            VALUES ($1, $2, $3, $4, $5, now())
            ON CONFLICT (tenant_id, variant_id) DO UPDATE
              SET fingerprint = EXCLUDED.fingerprint,
                  item_id = EXCLUDED.item_id,
                  updated_at = now()
            "#,
        )
        .bind(variant_id)
        .bind(item_id)
        .bind(&fingerprint)
        .bind(stock)
        .bind(avg_cost)
        .execute(tx.conn())
        .await?;

        keep_variants.push(variant_id);
    }

    // Drop matrix SKUs that are no longer in the product.
    let stale = sqlx::query_as::<_, (i32,)>(
        r#"
        SELECT variant_id FROM matrix_skus
        WHERE item_id = $1 AND NOT (variant_id = ANY($2))
        "#,
    )
    .bind(item_id)
    .bind(&keep_variants)
    .fetch_all(tx.conn())
    .await?;
    for (variant_id,) in stale {
        sqlx::query("DELETE FROM matrix_skus WHERE variant_id = $1")
            .bind(variant_id)
            .execute(tx.conn())
            .await?;
        sqlx::query("DELETE FROM item_variants WHERE id = $1 AND item_id = $2")
            .bind(variant_id)
            .bind(item_id)
            .execute(tx.conn())
            .await?;
    }

    get_recipe(tx, item_id).await
}

pub async fn patch_sku(
    tx: &mut TenantTx<'_>,
    variant_id: i32,
    body: &SkuPatch,
) -> Result<Value, AppError> {
    let owned: Option<(i32,)> =
        sqlx::query_as("SELECT item_id FROM matrix_skus WHERE variant_id = $1")
            .bind(variant_id)
            .fetch_optional(tx.conn())
            .await?;
    let Some((item_id,)) = owned else {
        return Err(AppError::NotFound("That matrix SKU was not found.".into()));
    };

    if let Some(rate) = body.rate {
        if rate < 1 {
            return Err(AppError::BadRequest("Enter a sale rate of at least 1.".into()));
        }
        sqlx::query("UPDATE item_variants SET rate = $2 WHERE id = $1")
            .bind(variant_id)
            .bind(rate)
            .execute(tx.conn())
            .await?;
    }
    if let Some(barcode) = &body.barcode {
        let clean = clean_code(barcode);
        sqlx::query("UPDATE item_variants SET barcode = $2 WHERE id = $1")
            .bind(variant_id)
            .bind(clean.as_deref())
            .execute(tx.conn())
            .await
            .map_err(barcode_error)?;
    }
    if body.stock.is_some() || body.avg_cost.is_some() {
        let stock = body.stock;
        let avg_cost = body.avg_cost;
        if let Some(stock) = stock {
            if stock < 0 {
                return Err(AppError::BadRequest("Stock cannot be negative.".into()));
            }
        }
        if let Some(avg_cost) = avg_cost {
            if avg_cost < 0 {
                return Err(AppError::BadRequest("Cost cannot be negative.".into()));
            }
        }
        sqlx::query(
            r#"
            UPDATE matrix_skus
            SET stock = COALESCE($2, stock),
                avg_cost = COALESCE($3, avg_cost),
                updated_at = now()
            WHERE variant_id = $1
            "#,
        )
        .bind(variant_id)
        .bind(stock)
        .bind(avg_cost)
        .execute(tx.conn())
        .await?;
    }

    let skus = load_skus(tx, Some(item_id)).await?;
    let sku = skus
        .into_iter()
        .find(|row| row.variant_id == variant_id)
        .ok_or_else(|| AppError::NotFound("That matrix SKU was not found.".into()))?;
    Ok(json!({ "sku": sku_json(sku) }))
}

pub async fn adjust_sku(
    tx: &mut TenantTx<'_>,
    variant_id: i32,
    body: &SkuAdjust,
) -> Result<Value, AppError> {
    if body.qty < 1 {
        return Err(AppError::BadRequest(
            "Enter a quantity of at least 1.".into(),
        ));
    }
    if body.qty > 1_000_000 {
        return Err(AppError::BadRequest(
            "Enter a quantity the till can store.".into(),
        ));
    }
    let inbound = match body.direction.as_str() {
        "in" => true,
        "out" => false,
        _ => return Err(AppError::BadRequest("Choose stock in or stock out.".into())),
    };
    let note = body
        .note
        .as_ref()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty());
    if note.is_some_and(|value| value.chars().count() > 80) {
        return Err(AppError::BadRequest(
            "Keep the note to 80 characters.".into(),
        ));
    }
    let label = if inbound { "Received" } else { "Removed" };
    let reason = match note {
        Some(value) => format!("{label} · {value}"),
        None => label.to_string(),
    };
    let delta = if inbound { body.qty } else { -body.qty };

    let held = sqlx::query_as::<_, (i32, i32, i64)>(
        "SELECT item_id, stock, avg_cost FROM matrix_skus WHERE variant_id = $1 FOR UPDATE",
    )
    .bind(variant_id)
    .fetch_optional(tx.conn())
    .await?;
    let Some((item_id, on_hand, average)) = held else {
        return Err(AppError::NotFound("That matrix SKU was not found.".into()));
    };
    if !inbound && on_hand < body.qty {
        return Err(AppError::BadRequest(format!("Only {on_hand} are on hand.")));
    }
    let next_cost = if body.update_cost {
        if !inbound {
            return Err(AppError::BadRequest("A purchase adds stock.".into()));
        }
        let Some(total) = body.total else {
            return Err(AppError::BadRequest(
                "Enter the total amount for this purchase.".into(),
            ));
        };
        if total < 0 || total > 1_000_000_000 {
            return Err(AppError::BadRequest(
                "Enter a purchase total the till can store.".into(),
            ));
        }
        Some(next_average(on_hand, average, body.qty, total))
    } else {
        None
    };

    sqlx::query(
        r#"
        UPDATE matrix_skus
        SET stock = stock + $1,
            avg_cost = COALESCE($2, avg_cost),
            updated_at = now()
        WHERE variant_id = $3
        "#,
    )
    .bind(delta)
    .bind(next_cost)
    .bind(variant_id)
    .execute(tx.conn())
    .await?;

    sqlx::query(
        r#"
        INSERT INTO matrix_sku_movements (variant_id, item_id, qty, reason)
        VALUES ($1, $2, $3, $4)
        "#,
    )
    .bind(variant_id)
    .bind(item_id)
    .bind(delta)
    .bind(&reason)
    .execute(tx.conn())
    .await?;

    let skus = load_skus(tx, Some(item_id)).await?;
    let sku = skus
        .into_iter()
        .find(|row| row.variant_id == variant_id)
        .ok_or_else(|| AppError::NotFound("That matrix SKU was not found.".into()))?;
    Ok(json!({ "sku": sku_json(sku) }))
}

pub async fn sku_summary(tx: &mut TenantTx<'_>) -> Result<Value, AppError> {
    let skus = load_skus(tx, None).await?;
    Ok(json!({
        "skus": skus.into_iter().map(sku_json).collect::<Vec<_>>(),
    }))
}

fn sku_json(row: SkuRow) -> Value {
    json!({
        "variant_id": row.variant_id,
        "item_id": row.item_id,
        "fingerprint": row.fingerprint,
        "name": row.name,
        "barcode": row.barcode,
        "rate": row.rate,
        "stock": row.stock,
        "avg_cost": row.avg_cost,
        "value": i64::from(row.stock).saturating_mul(row.avg_cost),
        "parent_name": row.parent_name,
        "section_id": row.section_id,
        "section_name": row.section_name,
        "accent": row.accent,
    })
}

async fn load_skus(
    tx: &mut TenantTx<'_>,
    item_id: Option<i32>,
) -> Result<Vec<SkuRow>, AppError> {
    let rows = sqlx::query_as::<_, SkuRow>(
        r#"
        SELECT
            s.variant_id,
            s.item_id,
            s.fingerprint,
            v.name,
            v.barcode,
            v.rate,
            s.stock,
            s.avg_cost,
            i.name AS parent_name,
            sec.name AS section_name,
            sec.id AS section_id,
            sec.accent
        FROM matrix_skus s
        JOIN item_variants v ON v.id = s.variant_id
        JOIN items i ON i.id = s.item_id
        JOIN sections sec ON sec.id = i.section_id
        WHERE ($1::int IS NULL OR s.item_id = $1)
        ORDER BY i.name, v.sort_order, v.id
        "#,
    )
    .bind(item_id)
    .fetch_all(tx.conn())
    .await?;
    Ok(rows)
}

async fn load_groups(tx: &mut TenantTx<'_>, item_id: i32) -> Result<Vec<Value>, AppError> {
    let groups = load_group_rows(tx, item_id).await?;
    Ok(groups
        .into_iter()
        .map(|group| {
            json!({
                "id": group.id,
                "name": group.name,
                "sort_order": group.sort_order,
                "values": group.values.into_iter().map(|value| json!({
                    "id": value.id,
                    "value": value.value,
                    "sort_order": value.sort_order,
                })).collect::<Vec<_>>(),
            })
        })
        .collect())
}

struct GroupRow {
    id: i32,
    name: String,
    sort_order: i32,
    values: Vec<ValueRow>,
}

struct ValueRow {
    id: i32,
    value: String,
    sort_order: i32,
}

async fn load_group_rows(tx: &mut TenantTx<'_>, item_id: i32) -> Result<Vec<GroupRow>, AppError> {
    let groups = sqlx::query_as::<_, (i32, String, i32)>(
        r#"
        SELECT id, name, sort_order
        FROM matrix_groups
        WHERE item_id = $1
        ORDER BY sort_order, id
        "#,
    )
    .bind(item_id)
    .fetch_all(tx.conn())
    .await?;
    let mut out = Vec::new();
    for (id, name, sort_order) in groups {
        let values = sqlx::query_as::<_, (i32, String, i32)>(
            r#"
            SELECT id, value, sort_order
            FROM matrix_group_values
            WHERE group_id = $1
            ORDER BY sort_order, id
            "#,
        )
        .bind(id)
        .fetch_all(tx.conn())
        .await?
        .into_iter()
        .map(|(id, value, sort_order)| ValueRow {
            id,
            value,
            sort_order,
        })
        .collect();
        out.push(GroupRow {
            id,
            name,
            sort_order,
            values,
        });
    }
    Ok(out)
}

fn validate_groups(groups: &[GroupInput]) -> Result<(), AppError> {
    if groups.len() > 8 {
        return Err(AppError::BadRequest(
            "Keep the matrix to 8 groups or fewer.".into(),
        ));
    }
    let mut names = std::collections::HashSet::new();
    for group in groups {
        let name = group.name.trim();
        if name.is_empty() {
            return Err(AppError::BadRequest("Enter a group name.".into()));
        }
        if name.chars().count() > 40 {
            return Err(AppError::BadRequest(
                "Keep group names to 40 characters.".into(),
            ));
        }
        if !names.insert(name.to_ascii_lowercase()) {
            return Err(AppError::BadRequest(format!(
                "The group {name} is listed twice."
            )));
        }
        if group.values.is_empty() {
            return Err(AppError::BadRequest(format!(
                "Add at least one value for {name}."
            )));
        }
        if group.values.len() > 100 {
            return Err(AppError::BadRequest(format!(
                "Keep {name} to 100 values or fewer."
            )));
        }
        let mut values = std::collections::HashSet::new();
        for value in &group.values {
            let text = value.value.trim();
            if text.is_empty() {
                return Err(AppError::BadRequest(format!(
                    "Enter a value for {name}."
                )));
            }
            if text.chars().count() > 40 {
                return Err(AppError::BadRequest(
                    "Keep values to 40 characters.".into(),
                ));
            }
            if !values.insert(text.to_ascii_lowercase()) {
                return Err(AppError::BadRequest(format!(
                    "{text} is listed twice under {name}."
                )));
            }
        }
    }
    Ok(())
}

fn cartesian(groups: &[GroupRow]) -> Vec<Vec<(String, String)>> {
    let mut combos: Vec<Vec<(String, String)>> = vec![vec![]];
    for group in groups {
        let mut next = Vec::new();
        for prefix in &combos {
            for value in &group.values {
                let mut row = prefix.clone();
                row.push((group.name.clone(), value.value.clone()));
                next.push(row);
            }
        }
        combos = next;
    }
    combos
}

fn fingerprint_for(combo: &[(String, String)]) -> String {
    combo
        .iter()
        .map(|(name, value)| format!("{name}={value}"))
        .collect::<Vec<_>>()
        .join("\u{1f}")
}

async fn ensure_item_exists(tx: &mut TenantTx<'_>, item_id: i32) -> Result<(), AppError> {
    let exists: Option<i32> = sqlx::query_scalar("SELECT id FROM items WHERE id = $1")
        .bind(item_id)
        .fetch_optional(tx.conn())
        .await?;
    if exists.is_none() {
        return Err(AppError::NotFound(
            "That item is no longer in the catalog.".into(),
        ));
    }
    Ok(())
}

async fn next_barcode_seq(tx: &mut TenantTx<'_>, prefix: &str) -> Result<u64, AppError> {
    let pattern = format!("{prefix}%");
    let existing: Vec<String> = sqlx::query_scalar(
        r#"
        SELECT barcode FROM item_variants
        WHERE barcode IS NOT NULL AND barcode LIKE $1
        "#,
    )
    .bind(&pattern)
    .fetch_all(tx.conn())
    .await?;
    let mut max = 0_u64;
    for barcode in existing {
        let suffix = barcode.strip_prefix(prefix).unwrap_or("");
        if let Ok(n) = suffix.parse::<u64>() {
            max = max.max(n);
        }
    }
    Ok(max + 1)
}

async fn allocate_barcode(
    tx: &mut TenantTx<'_>,
    prefix: &str,
    seq: &mut u64,
) -> Result<String, AppError> {
    for _ in 0..50 {
        let candidate = if prefix.is_empty() {
            format!("{seq:06}")
        } else {
            format!("{prefix}{seq}")
        };
        *seq += 1;
        let taken: Option<i32> = sqlx::query_scalar(
            r#"
            SELECT 1 FROM item_variants WHERE barcode = $1
            UNION ALL
            SELECT 1 FROM items WHERE barcode = $1
            LIMIT 1
            "#,
        )
        .bind(&candidate)
        .fetch_optional(tx.conn())
        .await?;
        if taken.is_none() {
            return Ok(candidate);
        }
    }
    Err(AppError::BadRequest(
        "Could not allocate a unique barcode. Change the prefix.".into(),
    ))
}

fn clean_code(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn barcode_error(err: sqlx::Error) -> AppError {
    let message = err.to_string();
    if message.contains("item_variants_barcode_key") || message.contains("items_barcode_key") {
        AppError::BadRequest("That barcode is already in use.".into())
    } else {
        AppError::from(err)
    }
}

fn next_average(on_hand: i32, average: i64, qty: i32, total_major: i64) -> i64 {
    let qty = i64::from(qty);
    let unit = (total_major * 100 + qty / 2) / qty;
    if on_hand <= 0 || average <= 0 {
        return unit;
    }
    let on_hand = i64::from(on_hand);
    let next = on_hand + qty;
    (on_hand * average + qty * unit + next / 2) / next
}
