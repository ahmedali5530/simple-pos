use sqlx::PgPool;

use crate::billing::load_pos;
use crate::error::AppError;
use crate::models::{PosSnapshot, ShopConfig};

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

#[derive(Debug, serde::Serialize, sqlx::FromRow)]
pub struct StockMovement {
    pub id: i32,
    pub item_id: i32,
    pub item_name: String,
    pub section_name: String,
    pub qty: i32,
    pub reason: String,
    pub bill_no: Option<String>,
    pub created_at: String,
}

#[derive(Debug, serde::Serialize)]
pub struct StockPage {
    pub pos: PosSnapshot,
    pub movements: Vec<StockMovement>,
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

#[derive(sqlx::FromRow)]
struct BalanceRow {
    id: i32,
    barcode: Option<String>,
    name: String,
    section_id: i32,
    section_name: String,
    accent: String,
    rate: i32,
    cost: i64,
    stock: i32,
    received: i64,
    removed: i64,
}

#[derive(Debug, serde::Serialize, sqlx::FromRow)]
pub struct SectionTotal {
    pub id: i32,
    pub name: String,
    pub short_name: String,
    pub accent: String,
    pub bills: i64,
    pub total: i64,
}

#[derive(Debug, serde::Serialize)]
pub struct BillLineView {
    pub name: String,
    pub qty: i32,
    pub amount: i32,
}

#[derive(Debug, serde::Serialize)]
pub struct BillView {
    pub id: i32,
    pub bill_no: String,
    pub section_name: String,
    pub short_name: String,
    pub accent: String,
    pub total: i32,
    pub cost: i64,
    pub created_at: String,
    pub cashier_name: Option<String>,
    pub customer: Option<String>,
    pub lines: Vec<BillLineView>,
}

#[derive(Debug, serde::Serialize)]
pub struct Reports {
    pub sections: Vec<SectionTotal>,
    pub bills: Vec<BillView>,
    pub movements: Vec<StockMovement>,
    pub bill_count: i64,
    pub total: i64,
    pub stock_units: i64,
    pub stock_value: i64,
    pub profit: i64,
    pub bills_truncated: bool,
    pub movements_truncated: bool,
    pub mix: Vec<MixLine>,
    pub mix_truncated: bool,
    pub range_label: String,
}

#[derive(Debug, serde::Serialize, sqlx::FromRow)]
pub struct MixLine {
    pub id: i32,
    pub barcode: Option<String>,
    pub name: String,
    pub section_name: String,
    pub qty_in: i64,
    pub qty_out: i64,
    pub opening: i64,
}

const MIX_LIMIT: i64 = 400;

#[derive(sqlx::FromRow)]
struct BillRow {
    id: i32,
    bill_no: String,
    section_name: String,
    short_name: String,
    accent: String,
    total: i32,
    created_at: String,
    cashier_name: Option<String>,
    customer: Option<String>,
}

#[derive(sqlx::FromRow)]
struct LineRow {
    bill_id: i32,
    name: String,
    qty: i32,
    amount: i32,
}

#[derive(sqlx::FromRow)]
struct CostRow {
    bill_id: i32,
    cost: i64,
}

const LIST_LIMIT: i64 = 200;

pub async fn adjust_stock(
    pool: &PgPool,
    shop: &ShopConfig,
    move_request: &StockMove,
) -> Result<StockPage, AppError> {
    if move_request.qty < 1 {
        return Err(AppError::BadRequest("Enter a quantity of at least 1.".into()));
    }
    if move_request.qty > 1_000_000 {
        return Err(AppError::BadRequest(
            "Enter a quantity the till can store.".into(),
        ));
    }
    let inbound = match move_request.direction.as_str() {
        "in" => true,
        "out" => false,
        _ => {
            return Err(AppError::BadRequest(
                "Choose stock in or stock out.".into(),
            ))
        }
    };
    let note = move_request
        .note
        .as_ref()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty());
    if note.is_some_and(|value| value.chars().count() > 80) {
        return Err(AppError::BadRequest("Keep the note to 80 characters.".into()));
    }
    let label = if inbound { "Received" } else { "Removed" };
    let reason = match note {
        Some(value) => format!("{label} · {value}"),
        None => label.to_string(),
    };
    let delta = if inbound { move_request.qty } else { -move_request.qty };

    let mut tx = pool.begin().await?;
    let held = sqlx::query_as::<_, (i32, i64)>(
        "SELECT stock, avg_cost FROM items WHERE id = $1 AND active FOR UPDATE",
    )
    .bind(move_request.item_id)
    .fetch_optional(&mut *tx)
    .await?;
    let Some((on_hand, average)) = held else {
        return Err(AppError::NotFound(
            "That item is no longer in the catalog.".into(),
        ));
    };
    if !inbound && on_hand < move_request.qty {
        return Err(AppError::BadRequest(format!(
            "Only {on_hand} are on hand."
        )));
    }
    let next_cost = if move_request.update_cost {
        if !inbound {
            return Err(AppError::BadRequest(
                "A purchase adds stock.".into(),
            ));
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
    sqlx::query("UPDATE items SET stock = stock + $1, avg_cost = COALESCE($2, avg_cost) WHERE id = $3")
        .bind(delta)
        .bind(next_cost)
        .bind(move_request.item_id)
        .execute(&mut *tx)
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
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;

    Ok(StockPage {
        pos: load_pos(pool, shop).await?,
        movements: movements(pool, Some(move_request.item_id), None, None, 40).await?,
    })
}

pub async fn stock_summary(pool: &PgPool) -> Result<Vec<StockBalance>, AppError> {
    let rows = sqlx::query_as::<_, BalanceRow>(
        r#"
        SELECT
            i.id,
            i.barcode,
            i.name,
            i.section_id,
            s.short_name AS section_name,
            s.accent,
            i.rate,
            i.avg_cost AS cost,
            i.stock,
            COALESCE(SUM(m.qty) FILTER (WHERE m.qty > 0), 0)::bigint AS received,
            COALESCE(SUM(-m.qty) FILTER (WHERE m.qty < 0), 0)::bigint AS removed
        FROM items i
        JOIN sections s ON s.id = i.section_id
        LEFT JOIN stock_movements m ON m.item_id = i.id
        WHERE i.active
        GROUP BY i.id, s.id
        ORDER BY s.pos_order, i.sort_order, i.id
        "#,
    )
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| {
            let net = row.received - row.removed;
            let opening = i64::from(row.stock) - net;
            let stock_in = row.received + opening.max(0);
            let stock_out = row.removed + (-opening).max(0);
            StockBalance {
                id: row.id,
                barcode: row.barcode,
                name: row.name,
                section_id: row.section_id,
                section_name: row.section_name,
                accent: row.accent,
                rate: row.rate,
                cost: row.cost,
                value: i64::from(row.stock).saturating_mul(row.cost),
                stock_in,
                stock_out,
                stock: row.stock,
            }
        })
        .collect())
}

pub async fn recent_movements(
    pool: &PgPool,
    item_id: Option<i32>,
) -> Result<Vec<StockMovement>, AppError> {
    movements(pool, item_id, None, None, 40).await
}

pub async fn load_reports(
    pool: &PgPool,
    from: Option<&str>,
    to: Option<&str>,
) -> Result<Reports, AppError> {
    check_range(pool, from, to).await?;
    let sections = sqlx::query_as::<_, SectionTotal>(
        r#"
        SELECT
            s.id,
            s.name,
            s.short_name,
            s.accent,
            COUNT(b.id)::bigint AS bills,
            COALESCE(SUM(b.total), 0)::bigint AS total
        FROM sections s
        LEFT JOIN bills b ON b.section_id = s.id
            AND ($1::text IS NULL OR b.created_at >= $1::timestamptz)
            AND ($2::text IS NULL OR b.created_at < $2::timestamptz)
        GROUP BY s.id
        ORDER BY s.pos_order, s.id
        "#,
    )
    .bind(from)
    .bind(to)
    .fetch_all(pool)
    .await?;

    let bill_count: i64 = sections.iter().map(|section| section.bills).sum();
    let total: i64 = sections.iter().map(|section| section.total).sum();
    let (stock_units, stock_value) = sqlx::query_as::<_, (i64, i64)>(
        r#"
        SELECT
            COALESCE(SUM(stock), 0)::bigint,
            COALESCE(SUM(stock::bigint * avg_cost), 0)::bigint
        FROM items
        WHERE active
        "#,
    )
    .fetch_one(pool)
    .await?;
    let goods = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COALESCE(SUM(
            CASE
                WHEN bl.cost_total <> 0 THEN bl.cost_total
                ELSE i.avg_cost * bl.qty::bigint * bl.units::bigint
            END
        ), 0)::bigint
        FROM bill_lines bl
        JOIN bills b ON b.id = bl.bill_id
        JOIN items i ON i.id = bl.item_id
        WHERE ($1::text IS NULL OR b.created_at >= $1::timestamptz)
          AND ($2::text IS NULL OR b.created_at < $2::timestamptz)
        "#,
    )
    .bind(from)
    .bind(to)
    .fetch_one(pool)
    .await?;
    let profit = total.saturating_mul(100) - goods;

    let rows = sqlx::query_as::<_, BillRow>(
        r#"
        SELECT
            b.id,
            b.bill_no,
            s.name AS section_name,
            s.short_name,
            s.accent,
            b.total,
            to_char(b.created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS"Z"') AS created_at,
            u.name AS cashier_name,
            b.customer
        FROM bills b
        JOIN sections s ON s.id = b.section_id
        LEFT JOIN users u ON u.id = b.user_id
        WHERE ($1::text IS NULL OR b.created_at >= $1::timestamptz)
          AND ($2::text IS NULL OR b.created_at < $2::timestamptz)
        ORDER BY b.created_at DESC, b.id DESC
        LIMIT 200
        "#,
    )
    .bind(from)
    .bind(to)
    .fetch_all(pool)
    .await?;

    let bill_ids: Vec<i32> = rows.iter().map(|bill| bill.id).collect();
    let line_rows = if bill_ids.is_empty() {
        Vec::new()
    } else {
        sqlx::query_as::<_, LineRow>(
            r#"
            SELECT bill_id, name, qty, amount
            FROM bill_lines
            WHERE bill_id = ANY($1)
            ORDER BY id
            "#,
        )
        .bind(&bill_ids)
        .fetch_all(pool)
        .await?
    };
    let cost_rows = if bill_ids.is_empty() {
        Vec::new()
    } else {
        sqlx::query_as::<_, CostRow>(
            r#"
            SELECT
                bl.bill_id,
                COALESCE(SUM(
                    CASE
                        WHEN bl.cost_total <> 0 THEN bl.cost_total
                        ELSE i.avg_cost * bl.qty::bigint * bl.units::bigint
                    END
                ), 0)::bigint AS cost
            FROM bill_lines bl
            JOIN items i ON i.id = bl.item_id
            WHERE bl.bill_id = ANY($1)
            GROUP BY bl.bill_id
            "#,
        )
        .bind(&bill_ids)
        .fetch_all(pool)
        .await?
    };

    let bills = rows
        .into_iter()
        .map(|bill| BillView {
            lines: line_rows
                .iter()
                .filter(|line| line.bill_id == bill.id)
                .map(|line| BillLineView {
                    name: line.name.clone(),
                    qty: line.qty,
                    amount: line.amount,
                })
                .collect(),
            id: bill.id,
            bill_no: bill.bill_no,
            section_name: bill.section_name,
            short_name: bill.short_name,
            accent: bill.accent,
            total: bill.total,
            cost: cost_rows
                .iter()
                .find(|row| row.bill_id == bill.id)
                .map(|row| row.cost)
                .unwrap_or(0),
            created_at: bill.created_at,
            cashier_name: bill.cashier_name,
            customer: bill.customer,
        })
        .collect();

    let movement_count = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COUNT(*)
        FROM stock_movements m
        WHERE ($1::text IS NULL OR m.created_at >= $1::timestamptz)
          AND ($2::text IS NULL OR m.created_at < $2::timestamptz)
        "#,
    )
    .bind(from)
    .bind(to)
    .fetch_one(pool)
    .await?;
    let movements = movements(pool, None, from, to, LIST_LIMIT).await?;
    let mix = product_mix(pool, from, to).await?;

    Ok(Reports {
        sections,
        bills_truncated: bill_count > LIST_LIMIT,
        movements_truncated: movement_count > LIST_LIMIT,
        bills,
        movements,
        bill_count,
        total,
        stock_units,
        stock_value,
        profit,
        mix_truncated: mix.truncated,
        range_label: mix.label,
        mix: mix.lines,
    })
}

pub struct ProductMix {
    pub lines: Vec<MixLine>,
    pub truncated: bool,
    pub label: String,
}

pub async fn product_mix(pool: &PgPool, from: Option<&str>, to: Option<&str>) -> Result<ProductMix, AppError> {
    check_range(pool, from, to).await?;
    let mut lines = sqlx::query_as::<_, MixLine>(
        r#"
        SELECT
            i.id,
            i.barcode,
            i.name,
            s.short_name AS section_name,
            COALESCE(SUM(CASE WHEN m.qty > 0 THEN m.qty ELSE 0 END), 0)::bigint AS qty_in,
            COALESCE(SUM(CASE WHEN m.qty < 0 THEN -m.qty ELSE 0 END), 0)::bigint AS qty_out,
            (
                i.stock::bigint - COALESCE((
                    SELECT SUM(earlier.qty)::bigint
                    FROM stock_movements earlier
                    WHERE earlier.item_id = i.id
                        AND ($1::text IS NULL OR earlier.created_at >= $1::timestamptz)
                ), 0)
            ) AS opening
        FROM items i
        JOIN sections s ON s.id = i.section_id
        JOIN stock_movements m ON m.item_id = i.id
            AND ($1::text IS NULL OR m.created_at >= $1::timestamptz)
            AND ($2::text IS NULL OR m.created_at < $2::timestamptz)
        WHERE i.active
        GROUP BY i.id, i.barcode, i.name, s.short_name, s.pos_order
        ORDER BY s.pos_order, i.name
        LIMIT $3
        "#,
    )
    .bind(from)
    .bind(to)
    .bind(MIX_LIMIT + 1)
    .fetch_all(pool)
    .await?;
    let truncated = lines.len() > MIX_LIMIT as usize;
    lines.truncate(MIX_LIMIT as usize);
    Ok(ProductMix {
        lines,
        truncated,
        label: range_label(pool, from, to).await?,
    })
}

async fn range_label(pool: &PgPool, from: Option<&str>, to: Option<&str>) -> Result<String, AppError> {
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
    .fetch_one(pool)
    .await?;
    Ok(label)
}

async fn movements(
    pool: &PgPool,
    item_id: Option<i32>,
    from: Option<&str>,
    to: Option<&str>,
    limit: i64,
) -> Result<Vec<StockMovement>, AppError> {
    sqlx::query_as::<_, StockMovement>(
        r#"
        SELECT
            m.id,
            m.item_id,
            i.name AS item_name,
            s.short_name AS section_name,
            m.qty,
            m.reason,
            b.bill_no,
            to_char(m.created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS"Z"') AS created_at
        FROM stock_movements m
        JOIN items i ON i.id = m.item_id
        JOIN sections s ON s.id = i.section_id
        LEFT JOIN bills b ON b.id = m.bill_id
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
    .fetch_all(pool)
    .await
    .map_err(AppError::from)
}

async fn check_range(pool: &PgPool, from: Option<&str>, to: Option<&str>) -> Result<(), AppError> {
    if from.is_none() && to.is_none() {
        return Ok(());
    }
    if from.is_none() || to.is_none() {
        return Err(AppError::BadRequest("Choose a start and an end.".into()));
    }
    sqlx::query("SELECT $1::timestamptz, $2::timestamptz")
        .bind(from)
        .bind(to)
        .execute(pool)
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
