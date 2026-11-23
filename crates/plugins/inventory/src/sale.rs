//! Stock taken by a posted sale. Safe to run more than once for the same bill.

use platform_kernel::error::AppError;
use sqlx::{Postgres, Transaction};

pub async fn apply_bill(tx: &mut Transaction<'_, Postgres>, bill_no: &str) -> Result<(), AppError> {
    let bill_id: Option<i32> = sqlx::query_scalar("SELECT id FROM bills WHERE bill_no = $1")
        .bind(bill_no)
        .fetch_optional(&mut **tx)
        .await?;
    let Some(bill_id) = bill_id else {
        return Ok(());
    };
    let already: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM stock_movements WHERE bill_id = $1)",
    )
    .bind(bill_id)
    .fetch_one(&mut **tx)
    .await?;
    if already {
        return Ok(());
    }
    let lines: Vec<(i32, i32)> = sqlx::query_as(
        r#"
        SELECT item_id, COALESCE(SUM(qty * units), 0)::int
        FROM bill_lines
        WHERE bill_id = $1
          AND reference = ''
        GROUP BY item_id
        "#,
    )
    .bind(bill_id)
    .fetch_all(&mut **tx)
    .await?;
    for (item_id, qty) in lines {
        if qty == 0 {
            continue;
        }
        sqlx::query("UPDATE items SET stock = stock - $1 WHERE id = $2")
            .bind(qty)
            .bind(item_id)
            .execute(&mut **tx)
            .await?;
        sqlx::query(
            r#"
            INSERT INTO stock_movements (item_id, qty, reason, bill_id)
            VALUES ($1, $2, $3, $4)
            "#,
        )
        .bind(item_id)
        .bind(-qty)
        .bind(format!("POS bill {bill_no}"))
        .bind(bill_id)
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}
