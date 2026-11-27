use std::collections::{HashMap, HashSet};

use catalog_contract::CatalogApi;

use platform_kernel::error::AppError;

#[derive(Debug, serde::Deserialize)]
pub struct StockMove {
    pub item_id: i32,
    pub qty: i32,
    pub direction: String,
    #[serde(default)]
    pub note: Option<String>,
    #[serde(default)]
    pub update_cost: bool,
    #[serde(default)]
    pub total: Option<i64>,
}

pub use inventory_contract::{MixLine, StockMovement};

#[derive(sqlx::FromRow)]
struct MovementRow {
    id: i32,
    item_id: i32,
    qty: i32,
    reason: String,
    bill_no: Option<String>,
    created_at: String,
}

#[derive(Debug, serde::Serialize)]
pub struct StockBalance {
    pub id: i32,
    pub barcode: Option<String>,
    pub name: String,
    pub section_id: i32,
    pub section_name: String,
    pub accent: String,
    pub rate: i32,
    pub cost: i64,
    pub value: i64,
    pub stock_in: i64,
    pub stock_out: i64,
    pub stock: i32,
}

const MIX_LIMIT: i64 = 400;

const LIST_LIMIT: i64 = 200;

pub async fn adjust_stock(
    tx: &mut platform_kernel::context::TenantTx<'_>,
    catalog: &CatalogApi,
    move_request: &StockMove,
) -> Result<Vec<StockMovement>, AppError> {
    if move_request.qty < 1 {
        return Err(AppError::BadRequest(
            "Enter a quantity of at least 1.".into(),
        ));
    }
    if move_request.qty > 1_000_000 {
        return Err(AppError::BadRequest(
            "Enter a quantity the till can store.".into(),
        ));
    }
    let inbound = match move_request.direction.as_str() {
        "in" => true,
        "out" => false,
        _ => return Err(AppError::BadRequest("Choose stock in or stock out.".into())),
    };
    let note = move_request
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
    let delta = if inbound {
        move_request.qty
    } else {
        -move_request.qty
    };

    if catalog.0.get_item(tx, move_request.item_id).await?.is_none() {
        return Err(AppError::NotFound(
            "That item is no longer in the catalog.".into(),
        ));
    }
    sqlx::query(
        r#"
        INSERT INTO inventory_balances (tenant_id, item_id, stock, avg_cost)
        VALUES (current_setting('app.tenant_id')::uuid, $1, 0, 0)
        ON CONFLICT (tenant_id, item_id) DO NOTHING
        "#,
    )
    .bind(move_request.item_id)
    .execute(tx.conn())
    .await?;
    let held = sqlx::query_as::<_, (i32, i64)>(
        "SELECT stock, avg_cost FROM inventory_balances WHERE item_id = $1 FOR UPDATE",
    )
    .bind(move_request.item_id)
    .fetch_optional(tx.conn())
    .await?;
    let Some((on_hand, average)) = held else {
        return Err(AppError::NotFound(
            "That item is no longer in the catalog.".into(),
        ));
    };
    if !inbound && on_hand < move_request.qty {
        return Err(AppError::BadRequest(format!("Only {on_hand} are on hand.")));
    }
    let next_cost = if move_request.update_cost {
        if !inbound {
            return Err(AppError::BadRequest("A purchase adds stock.".into()));
        }
        let Some(total) = move_request.total else {
            return Err(AppError::BadRequest(
                "Enter the total amount for this purchase.".into(),
            ));
        };
        if total < 0 || total > 1_000_000_000 {
            return Err(AppError::BadRequest(
                "Enter a purchase total the till can store.".into(),
            ));
        }
        Some(next_average(on_hand, average, move_request.qty, total))
    } else {
        None
    };
    sqlx::query(
        r#"
        UPDATE inventory_balances
        SET stock = stock + $1,
            avg_cost = COALESCE($2, avg_cost)
        WHERE item_id = $3
        "#,
    )
    .bind(delta)
    .bind(next_cost)
    .bind(move_request.item_id)
    .execute(tx.conn())
    .await?;
    sqlx::query(
        r#"
        INSERT INTO stock_movements (item_id, qty, reason)
        VALUES ($1, $2, $3)
        "#,
    )
    .bind(move_request.item_id)
    .bind(delta)
    .bind(&reason)
    .execute(tx.conn())
    .await?;
    movements_tx(tx, catalog, Some(move_request.item_id), None, None, 40).await
}

pub async fn stock_summary(
    tx: &mut platform_kernel::context::TenantTx<'_>,
    catalog: &CatalogApi,
) -> Result<Vec<StockBalance>, AppError> {
    let snapshot = catalog.0.snapshot(tx).await?;
    let totals: Vec<(i32, i64, i64)> = sqlx::query_as(
        r#"
        SELECT item_id,
               COALESCE(SUM(qty) FILTER (WHERE qty > 0), 0)::bigint,
               COALESCE(SUM(-qty) FILTER (WHERE qty < 0), 0)::bigint
        FROM stock_movements
        GROUP BY item_id
        "#,
    )
    .fetch_all(tx.conn())
    .await?;
    let totals: HashMap<i32, (i64, i64)> = totals
        .into_iter()
        .map(|(item_id, received, removed)| (item_id, (received, removed)))
        .collect();
    let sections: HashMap<i32, (String, String, i32)> = snapshot
        .sections
        .iter()
        .map(|section| {
            (
                section.id,
                (
                    section.short_name.clone(),
                    section.accent.clone(),
                    section.pos_order,
                ),
            )
        })
        .collect();
    let order: HashMap<i32, usize> = snapshot
        .items
        .iter()
        .enumerate()
        .map(|(index, item)| (item.id, index))
        .collect();
    let mut rows: Vec<StockBalance> = snapshot
        .items
        .iter()
        .map(|item| {
            let (received, removed) = totals.get(&item.id).copied().unwrap_or((0, 0));
            let net = received - removed;
            let opening = i64::from(item.stock) - net;
            let stock_in = received + opening.max(0);
            let stock_out = removed + (-opening).max(0);
            let (section_name, accent, _) = sections
                .get(&item.section_id)
                .cloned()
                .unwrap_or_else(|| (String::new(), String::new(), 0));
            StockBalance {
                id: item.id,
                barcode: item.barcode.clone(),
                name: item.name.clone(),
                section_id: item.section_id,
                section_name,
                accent,
                rate: item.rate,
                cost: item.cost,
                value: i64::from(item.stock).saturating_mul(item.cost),
                stock_in,
                stock_out,
                stock: item.stock,
            }
        })
        .collect();
    rows.sort_by_key(|row| {
        (
            sections
                .get(&row.section_id)
                .map(|(_, _, pos_order)| *pos_order)
                .unwrap_or(0),
            order.get(&row.id).copied().unwrap_or(usize::MAX),
        )
    });
    Ok(rows)
}

pub async fn recent_movements(
    tx: &mut platform_kernel::context::TenantTx<'_>,
    catalog: &CatalogApi,
    item_id: Option<i32>,
) -> Result<Vec<StockMovement>, AppError> {
    movements_tx(tx, catalog, item_id, None, None, 40).await
}

pub async fn stock_report(
    tx: &mut platform_kernel::context::TenantTx<'_>,
    catalog: &CatalogApi,
    from: Option<&str>,
    to: Option<&str>,
) -> Result<inventory_contract::StockReport, AppError> {
    check_range_tx(tx, from, to).await?;
    let snapshot = catalog.0.snapshot(tx).await?;
    let matrix_item_ids = matrix_item_ids(tx).await?;
    let stock_units = snapshot
        .items
        .iter()
        .filter(|item| !matrix_item_ids.contains(&item.id))
        .map(|item| i64::from(item.stock))
        .sum::<i64>()
        + matrix_stock_units(tx).await?;
    let stock_value = snapshot
        .items
        .iter()
        .filter(|item| !matrix_item_ids.contains(&item.id))
        .map(|item| i64::from(item.stock).saturating_mul(item.cost))
        .sum::<i64>()
        + matrix_stock_value(tx).await?;
    let mut movement_count = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COUNT(*)
        FROM stock_movements m
        WHERE ($1::text IS NULL OR m.created_at >= $1::timestamptz)
          AND ($2::text IS NULL OR m.created_at < $2::timestamptz)
        "#,
    )
    .bind(from)
    .bind(to)
    .fetch_one(tx.conn())
    .await?;
    if matrix_tables_present(tx).await? {
        movement_count += sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*)
            FROM matrix_sku_movements m
            WHERE ($1::text IS NULL OR m.created_at >= $1::timestamptz)
              AND ($2::text IS NULL OR m.created_at < $2::timestamptz)
            "#,
        )
        .bind(from)
        .bind(to)
        .fetch_one(tx.conn())
        .await?;
    }
    let movements = movements_tx(tx, catalog, None, from, to, LIST_LIMIT).await?;
    let mix = product_mix_tx(tx, catalog, from, to).await?;
    Ok(inventory_contract::StockReport {
        movements,
        stock_units,
        stock_value,
        movements_truncated: movement_count > LIST_LIMIT,
        mix: mix.lines,
        mix_truncated: mix.truncated,
        range_label: mix.label,
    })
}

pub struct ProductMix {
    pub lines: Vec<MixLine>,
    pub truncated: bool,
    pub label: String,
}

pub async fn product_mix(
    tx: &mut platform_kernel::context::TenantTx<'_>,
    catalog: &CatalogApi,
    from: Option<&str>,
    to: Option<&str>,
) -> Result<ProductMix, AppError> {
    product_mix_tx(tx, catalog, from, to).await
}

async fn product_mix_tx(
    tx: &mut platform_kernel::context::TenantTx<'_>,
    catalog: &CatalogApi,
    from: Option<&str>,
    to: Option<&str>,
) -> Result<ProductMix, AppError> {
    check_range_tx(tx, from, to).await?;
    let snapshot = catalog.0.snapshot(tx).await?;
    let matrix_item_ids = matrix_item_ids(tx).await?;
    let range: Vec<(i32, i64, i64)> = sqlx::query_as(
        r#"
        SELECT item_id,
               COALESCE(SUM(CASE WHEN qty > 0 THEN qty ELSE 0 END), 0)::bigint,
               COALESCE(SUM(CASE WHEN qty < 0 THEN -qty ELSE 0 END), 0)::bigint
        FROM stock_movements
        WHERE ($1::text IS NULL OR created_at >= $1::timestamptz)
          AND ($2::text IS NULL OR created_at < $2::timestamptz)
        GROUP BY item_id
        "#,
    )
    .bind(from)
    .bind(to)
    .fetch_all(tx.conn())
    .await?;
    let range: HashMap<i32, (i64, i64)> = range
        .into_iter()
        .map(|(item_id, qty_in, qty_out)| (item_id, (qty_in, qty_out)))
        .collect();
    let later: Vec<(i32, i64)> = sqlx::query_as(
        r#"
        SELECT item_id, COALESCE(SUM(qty), 0)::bigint
        FROM stock_movements
        WHERE ($1::text IS NULL OR created_at >= $1::timestamptz)
        GROUP BY item_id
        "#,
    )
    .bind(from)
    .fetch_all(tx.conn())
    .await?;
    let later: HashMap<i32, i64> = later.into_iter().collect();
    let sections: HashMap<i32, (String, i32)> = snapshot
        .sections
        .iter()
        .map(|section| (section.id, (section.short_name.clone(), section.pos_order)))
        .collect();
    let mut sort_order: HashMap<i32, i32> = snapshot
        .items
        .iter()
        .map(|item| {
            (
                item.id,
                sections
                    .get(&item.section_id)
                    .map(|(_, pos_order)| *pos_order)
                    .unwrap_or(0),
            )
        })
        .collect();
    let mut lines: Vec<MixLine> = snapshot
        .items
        .iter()
        .filter(|item| !matrix_item_ids.contains(&item.id))
        .filter_map(|item| {
            let (qty_in, qty_out) = *range.get(&item.id)?;
            let (section_name, _) = sections
                .get(&item.section_id)
                .cloned()
                .unwrap_or_else(|| (String::new(), 0));
            Some(MixLine {
                id: item.id,
                barcode: item.barcode.clone(),
                name: item.name.clone(),
                section_name,
                qty_in,
                qty_out,
                opening: i64::from(item.stock) - later.get(&item.id).copied().unwrap_or(0),
            })
        })
        .collect();
    if matrix_tables_present(tx).await? {
        #[derive(sqlx::FromRow)]
        struct MatrixMixRow {
            variant_id: i32,
            item_id: i32,
            stock: i32,
            variant_name: String,
            barcode: Option<String>,
            parent_name: String,
            section_id: i32,
            qty_in: i64,
            qty_out: i64,
        }
        let matrix_range: Vec<MatrixMixRow> = sqlx::query_as(
            r#"
            SELECT
                s.variant_id,
                s.item_id,
                s.stock,
                v.name AS variant_name,
                v.barcode,
                i.name AS parent_name,
                i.section_id,
                COALESCE(SUM(CASE WHEN m.qty > 0 THEN m.qty ELSE 0 END), 0)::bigint AS qty_in,
                COALESCE(SUM(CASE WHEN m.qty < 0 THEN -m.qty ELSE 0 END), 0)::bigint AS qty_out
            FROM matrix_skus s
            JOIN item_variants v ON v.id = s.variant_id
            JOIN items i ON i.id = s.item_id
            JOIN matrix_sku_movements m ON m.variant_id = s.variant_id
            WHERE ($1::text IS NULL OR m.created_at >= $1::timestamptz)
              AND ($2::text IS NULL OR m.created_at < $2::timestamptz)
            GROUP BY s.variant_id, s.item_id, s.stock, v.name, v.barcode, i.name, i.section_id
            "#,
        )
        .bind(from)
        .bind(to)
        .fetch_all(tx.conn())
        .await?;
        let matrix_later: Vec<(i32, i64)> = sqlx::query_as(
            r#"
            SELECT variant_id, COALESCE(SUM(qty), 0)::bigint
            FROM matrix_sku_movements
            WHERE ($1::text IS NULL OR created_at >= $1::timestamptz)
            GROUP BY variant_id
            "#,
        )
        .bind(from)
        .fetch_all(tx.conn())
        .await?;
        let matrix_later: HashMap<i32, i64> = matrix_later.into_iter().collect();
        for row in matrix_range {
            let line_id = -row.variant_id;
            let order = sections
                .get(&row.section_id)
                .map(|(_, pos_order)| *pos_order)
                .unwrap_or(0);
            sort_order.insert(line_id, order);
            let (section_name, _) = sections
                .get(&row.section_id)
                .cloned()
                .unwrap_or_else(|| (String::new(), 0));
            lines.push(MixLine {
                id: line_id,
                barcode: row.barcode,
                name: format!("{} · {}", row.parent_name, row.variant_name),
                section_name,
                qty_in: row.qty_in,
                qty_out: row.qty_out,
                opening: i64::from(row.stock)
                    - matrix_later.get(&row.variant_id).copied().unwrap_or(0),
            });
            let _ = row.item_id;
        }
    }
    lines.sort_by(|a, b| {
        sort_order
            .get(&a.id)
            .copied()
            .unwrap_or(0)
            .cmp(&sort_order.get(&b.id).copied().unwrap_or(0))
            .then_with(|| a.name.cmp(&b.name))
            .then_with(|| a.id.cmp(&b.id))
    });
    let truncated = lines.len() > MIX_LIMIT as usize;
    lines.truncate(MIX_LIMIT as usize);
    Ok(ProductMix {
        lines,
        truncated,
        label: range_label_tx(tx, from, to).await?,
    })
}

async fn range_label_tx(
    tx: &mut platform_kernel::context::TenantTx<'_>,
    from: Option<&str>,
    to: Option<&str>,
) -> Result<String, AppError> {
    if from.is_none() || to.is_none() {
        return Ok("All dates".into());
    }
    let label = sqlx::query_scalar::<_, String>(
        r#"
        SELECT to_char($1::timestamptz AT TIME ZONE 'Asia/Karachi', 'DD Mon YYYY')
            || ' – ' ||
            to_char(($2::timestamptz - interval '1 second') AT TIME ZONE 'Asia/Karachi', 'DD Mon YYYY')
        "#,
    )
    .bind(from)
    .bind(to)
    .fetch_one(tx.conn())
    .await?;
    Ok(label)
}

async fn movements_tx(
    tx: &mut platform_kernel::context::TenantTx<'_>,
    catalog: &CatalogApi,
    item_id: Option<i32>,
    from: Option<&str>,
    to: Option<&str>,
    limit: i64,
) -> Result<Vec<StockMovement>, AppError> {
    let snapshot = catalog.0.snapshot(tx).await?;
    let items: HashMap<i32, String> = snapshot
        .items
        .iter()
        .map(|item| (item.id, item.name.clone()))
        .collect();
    let sections: HashMap<i32, String> = snapshot
        .sections
        .iter()
        .map(|section| (section.id, section.short_name.clone()))
        .collect();
    let item_sections: HashMap<i32, i32> = snapshot
        .items
        .iter()
        .map(|item| (item.id, item.section_id))
        .collect();
    let rows = sqlx::query_as::<_, MovementRow>(
        r#"
        SELECT
            m.id,
            m.item_id,
            m.qty,
            m.reason,
            m.bill_no,
            to_char(m.created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS"Z"') AS created_at
        FROM stock_movements m
        WHERE ($1::int IS NULL OR m.item_id = $1)
          AND ($2::text IS NULL OR m.created_at >= $2::timestamptz)
          AND ($3::text IS NULL OR m.created_at < $3::timestamptz)
        ORDER BY m.created_at DESC, m.id DESC
        LIMIT $4
        "#,
    )
    .bind(item_id)
    .bind(from)
    .bind(to)
    .bind(limit)
    .fetch_all(tx.conn())
    .await?;
    let mut movements: Vec<StockMovement> = rows
        .into_iter()
        .map(|row| StockMovement {
            id: row.id,
            item_id: row.item_id,
            item_name: items
                .get(&row.item_id)
                .cloned()
                .unwrap_or_else(|| "Unknown item".into()),
            section_name: item_sections
                .get(&row.item_id)
                .and_then(|section_id| sections.get(section_id))
                .cloned()
                .unwrap_or_default(),
            qty: row.qty,
            reason: row.reason,
            bill_no: row.bill_no,
            created_at: row.created_at,
        })
        .collect();
    if matrix_tables_present(tx).await? {
        #[derive(sqlx::FromRow)]
        struct MatrixMoveRow {
            id: i32,
            item_id: i32,
            qty: i32,
            reason: String,
            created_at: String,
            parent_name: String,
            variant_name: String,
            section_id: i32,
        }
        let matrix_rows = sqlx::query_as::<_, MatrixMoveRow>(
            r#"
            SELECT
                m.id,
                m.item_id,
                m.qty,
                m.reason,
                to_char(m.created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS"Z"') AS created_at,
                i.name AS parent_name,
                v.name AS variant_name,
                i.section_id
            FROM matrix_sku_movements m
            JOIN matrix_skus s ON s.variant_id = m.variant_id
            JOIN item_variants v ON v.id = m.variant_id
            JOIN items i ON i.id = m.item_id
            WHERE ($1::int IS NULL OR m.item_id = $1)
              AND ($2::text IS NULL OR m.created_at >= $2::timestamptz)
              AND ($3::text IS NULL OR m.created_at < $3::timestamptz)
            ORDER BY m.created_at DESC, m.id DESC
            LIMIT $4
            "#,
        )
        .bind(item_id)
        .bind(from)
        .bind(to)
        .bind(limit)
        .fetch_all(tx.conn())
        .await?;
        movements.extend(matrix_rows.into_iter().map(|row| StockMovement {
            id: -row.id,
            item_id: row.item_id,
            item_name: format!("{} · {}", row.parent_name, row.variant_name),
            section_name: sections.get(&row.section_id).cloned().unwrap_or_default(),
            qty: row.qty,
            reason: row.reason,
            bill_no: None,
            created_at: row.created_at,
        }));
        movements.sort_by(|a, b| {
            b.created_at
                .cmp(&a.created_at)
                .then_with(|| b.id.abs().cmp(&a.id.abs()))
        });
        movements.truncate(limit as usize);
    }
    Ok(movements)
}

async fn matrix_tables_present(
    tx: &mut platform_kernel::context::TenantTx<'_>,
) -> Result<bool, AppError> {
    let present = sqlx::query_scalar::<_, bool>(
        "SELECT to_regclass('public.matrix_skus') IS NOT NULL",
    )
    .fetch_one(tx.conn())
    .await?;
    Ok(present)
}

async fn matrix_item_ids(
    tx: &mut platform_kernel::context::TenantTx<'_>,
) -> Result<HashSet<i32>, AppError> {
    if !matrix_tables_present(tx).await? {
        return Ok(HashSet::new());
    }
    let ids = sqlx::query_scalar::<_, i32>("SELECT DISTINCT item_id FROM matrix_skus")
        .fetch_all(tx.conn())
        .await?;
    Ok(ids.into_iter().collect())
}

async fn matrix_stock_units(
    tx: &mut platform_kernel::context::TenantTx<'_>,
) -> Result<i64, AppError> {
    if !matrix_tables_present(tx).await? {
        return Ok(0);
    }
    let units = sqlx::query_scalar::<_, i64>("SELECT COALESCE(SUM(stock), 0)::bigint FROM matrix_skus")
        .fetch_one(tx.conn())
        .await?;
    Ok(units)
}

async fn matrix_stock_value(
    tx: &mut platform_kernel::context::TenantTx<'_>,
) -> Result<i64, AppError> {
    if !matrix_tables_present(tx).await? {
        return Ok(0);
    }
    let value = sqlx::query_scalar::<_, i64>(
        "SELECT COALESCE(SUM(stock::bigint * avg_cost), 0)::bigint FROM matrix_skus",
    )
    .fetch_one(tx.conn())
    .await?;
    Ok(value)
}

async fn check_range_tx(
    tx: &mut platform_kernel::context::TenantTx<'_>,
    from: Option<&str>,
    to: Option<&str>,
) -> Result<(), AppError> {
    if from.is_none() && to.is_none() {
        return Ok(());
    }
    if from.is_none() || to.is_none() {
        return Err(AppError::BadRequest("Choose a start and an end.".into()));
    }
    sqlx::query("SELECT $1::timestamptz, $2::timestamptz")
        .bind(from)
        .bind(to)
        .execute(tx.conn())
        .await
        .map_err(|_| AppError::BadRequest("That date range could not be read.".into()))?;
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::next_average;

    #[test]
    fn purchase_sets_average_when_nothing_is_on_hand() {
        assert_eq!(next_average(0, 0, 3, 1000), 33333);
    }

    #[test]
    fn purchase_weights_the_stock_already_on_hand() {
        assert_eq!(next_average(10, 10000, 10, 200), 6000);
    }

    #[test]
    fn first_known_cost_replaces_an_unset_average() {
        assert_eq!(next_average(78, 0, 3, 1000), 33333);
    }
}
