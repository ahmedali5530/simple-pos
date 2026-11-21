use sqlx::PgPool;

use crate::error::AppError;

#[derive(Debug, serde::Deserialize)]
pub struct OpenShift {
    pub opening_cash: i64,
    pub petty_cash: i64,
}

#[derive(Debug, serde::Deserialize)]
pub struct ExpenseInput {
    pub description: String,
    pub category: String,
    pub amount: i64,
}

#[derive(Debug, serde::Deserialize)]
pub struct ShiftForm {
    #[serde(default)]
    pub expenses: Vec<ExpenseInput>,
    pub cash_taken: i64,
    pub manual_count: Option<i64>,
}

#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow)]
pub struct CashExpense {
    pub description: String,
    pub category: String,
    pub amount: i32,
}

const CATEGORIES: [&str; 7] = ["Supplies", "Wages", "Food", "Transport", "Utilities", "Maintenance", "Other"];

#[derive(Debug, serde::Serialize, sqlx::FromRow)]
pub struct CashShift {
    pub id: i32,
    pub opened_by: String,
    pub closed_by: Option<String>,
    pub opened_at: String,
    pub closed_at: Option<String>,
    pub opening_cash: i32,
    pub petty_cash: i32,
    pub sales: i64,
    pub expenses: Option<i32>,
    pub cash_taken: Option<i32>,
    pub manual_count: Option<i32>,
    pub system_count: Option<i64>,
    pub difference: Option<i64>,
    pub cash_left: Option<i32>,
    #[sqlx(skip)]
    #[serde(default)]
    pub lines: Vec<CashExpense>,
}

#[derive(Debug, serde::Serialize)]
pub struct CashDesk {
    pub open: Option<CashShift>,
    pub suggested_opening: i32,
    pub history: Vec<CashShift>,
}

pub async fn cash_desk(pool: &PgPool) -> Result<CashDesk, AppError> {
    let mut tx = crate::tenant::begin(pool).await?;
    let rows = shifts_tx(&mut tx, 40).await?;
    let suggested_opening = sqlx::query_scalar::<_, i32>(
        r#"
        SELECT cash_left
        FROM cash_shifts
        WHERE closed_at IS NOT NULL AND cash_left IS NOT NULL
        ORDER BY closed_at DESC, id DESC
        LIMIT 1
        "#,
    )
    .fetch_optional(&mut *tx)
    .await?
    .unwrap_or(0);
    let mut open = None;
    let mut history = Vec::new();
    for row in rows {
        if row.closed_at.is_none() && open.is_none() {
            open = Some(row);
        } else if row.closed_at.is_some() {
            history.push(row);
        }
    }
    tx.commit().await?;
    Ok(CashDesk {
        open,
        suggested_opening,
        history,
    })
}

pub async fn open_shift(pool: &PgPool, user_id: i32, form: &OpenShift) -> Result<CashDesk, AppError> {
    let opening_cash = whole(form.opening_cash, "Previous till cash")?;
    let petty_cash = whole(form.petty_cash, "Petty cash")?;
    let mut tx = crate::tenant::begin(pool).await?;
    let inserted = sqlx::query(
        r#"
        INSERT INTO cash_shifts (opened_by, opening_cash, petty_cash)
        SELECT $1, $2, $3
        WHERE NOT EXISTS (SELECT 1 FROM cash_shifts WHERE closed_at IS NULL)
        "#,
    )
    .bind(user_id)
    .bind(opening_cash)
    .bind(petty_cash)
    .execute(&mut *tx)
    .await?;
    if inserted.rows_affected() == 0 {
        return Err(AppError::BadRequest("A shift is already open.".into()));
    }
    tx.commit().await?;
    cash_desk(pool).await
}

pub async fn save_shift(pool: &PgPool, user_id: i32, form: &ShiftForm, close: bool) -> Result<CashDesk, AppError> {
    let lines = parse_lines(&form.expenses)?;
    let expense_total = whole(lines.iter().map(|line| i64::from(line.amount)).sum(), "Expenses")?;
    let cash_taken = whole(form.cash_taken, "Cash taken out")?;
    let manual_count = match form.manual_count {
        Some(value) => Some(whole(value, "Manual count")?),
        None if close => return Err(AppError::BadRequest("Enter the manual count before closing the day.".into())),
        None => None,
    };
    let mut tx = crate::tenant::begin(pool).await?;
    let locked = sqlx::query_as::<_, (i32, i32, i32)>(
        "SELECT id, opening_cash, petty_cash FROM cash_shifts WHERE closed_at IS NULL FOR UPDATE",
    )
    .fetch_optional(&mut *tx)
    .await?;
    let Some((id, opening_cash, petty_cash)) = locked else {
        return Err(AppError::BadRequest("Start the day before closing the till.".into()));
    };
    sqlx::query("DELETE FROM cash_expenses WHERE shift_id = $1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    for (index, line) in lines.iter().enumerate() {
        sqlx::query(
            r#"
            INSERT INTO cash_expenses (shift_id, description, category, amount, sort_order)
            VALUES ($1, $2, $3, $4, $5)
            "#,
        )
        .bind(id)
        .bind(&line.description)
        .bind(&line.category)
        .bind(line.amount)
        .bind(index as i32)
        .execute(&mut *tx)
        .await?;
    }
    let sales = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COALESCE(SUM(total), 0)::bigint
        FROM bills
        WHERE created_at >= (SELECT opened_at FROM cash_shifts WHERE id = $1)
        "#,
    )
    .bind(id)
    .fetch_one(&mut *tx)
    .await?;
    let system = i64::from(opening_cash) + i64::from(petty_cash) + sales - i64::from(expense_total) - i64::from(cash_taken);
    if close && system < 0 {
        return Err(AppError::BadRequest(
            "Cash left would be below zero. Check the expenses and the cash taken out.".into(),
        ));
    }
    if close {
        let cash_left = i32::try_from(system)
            .map_err(|_| AppError::BadRequest("Cash left is too large.".into()))?;
        let manual_count = manual_count.unwrap_or(0);
        let difference = i64::from(manual_count) - system;
        sqlx::query(
            r#"
            UPDATE cash_shifts
            SET closed_by = $2,
                closed_at = now(),
                sales = $3,
                expenses = $4,
                cash_taken = $5,
                manual_count = $6,
                system_count = $7,
                difference = $8,
                cash_left = $9
            WHERE id = $1 AND closed_at IS NULL
            "#,
        )
        .bind(id)
        .bind(user_id)
        .bind(sales)
        .bind(expense_total)
        .bind(cash_taken)
        .bind(manual_count)
        .bind(system)
        .bind(difference)
        .bind(cash_left)
        .execute(&mut *tx)
        .await?;
    } else {
        sqlx::query(
            r#"
            UPDATE cash_shifts
            SET expenses = $2,
                cash_taken = $3,
                manual_count = $4
            WHERE id = $1 AND closed_at IS NULL
            "#,
        )
        .bind(id)
        .bind(expense_total)
        .bind(cash_taken)
        .bind(manual_count)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    cash_desk(pool).await
}

pub async fn load_shift(pool: &PgPool, id: i32) -> Result<CashShift, AppError> {
    let mut tx = crate::tenant::begin(pool).await?;
    let shift = shifts_tx(&mut tx, 200)
        .await?
        .into_iter()
        .find(|shift| shift.id == id)
        .ok_or_else(|| AppError::NotFound("That closing is not in the till.".into()))?;
    tx.commit().await?;
    Ok(shift)
}

async fn shifts_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    limit: i64,
) -> Result<Vec<CashShift>, AppError> {
    let rows = sqlx::query_as::<_, CashShift>(
        r#"
        SELECT
            c.id,
            opener.name AS opened_by,
            closer.name AS closed_by,
            to_char(c.opened_at AT TIME ZONE 'Asia/Karachi', 'DD Mon YYYY, HH12:MI AM') AS opened_at,
            to_char(c.closed_at AT TIME ZONE 'Asia/Karachi', 'DD Mon YYYY, HH12:MI AM') AS closed_at,
            c.opening_cash,
            c.petty_cash,
            CASE
                WHEN c.closed_at IS NULL THEN (
                    SELECT COALESCE(SUM(b.total), 0)::bigint
                    FROM bills b
                    WHERE b.created_at >= c.opened_at
                )
                ELSE c.sales
            END AS sales,
            c.expenses,
            c.cash_taken,
            c.manual_count,
            c.system_count,
            c.difference,
            c.cash_left
        FROM cash_shifts c
        JOIN users opener ON opener.id = c.opened_by
        LEFT JOIN users closer ON closer.id = c.closed_by
        ORDER BY c.opened_at DESC, c.id DESC
        LIMIT $1
        "#,
    )
    .bind(limit)
    .fetch_all(&mut **tx)
    .await
    .map_err(AppError::from)?;
    attach_lines_tx(tx, rows).await
}

async fn attach_lines_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    mut rows: Vec<CashShift>,
) -> Result<Vec<CashShift>, AppError> {
    if rows.is_empty() {
        return Ok(rows);
    }
    let ids: Vec<i32> = rows.iter().map(|shift| shift.id).collect();
    let lines = sqlx::query_as::<_, (i32, String, String, i32)>(
        r#"
        SELECT shift_id, description, category, amount
        FROM cash_expenses
        WHERE shift_id = ANY($1)
        ORDER BY shift_id, sort_order, id
        "#,
    )
    .bind(&ids)
    .fetch_all(&mut **tx)
    .await?;
    for (shift_id, description, category, amount) in lines {
        if let Some(shift) = rows.iter_mut().find(|shift| shift.id == shift_id) {
            shift.lines.push(CashExpense {
                description,
                category,
                amount,
            });
        }
    }
    Ok(rows)
}

struct ParsedLine {
    description: String,
    category: String,
    amount: i32,
}

fn parse_lines(inputs: &[ExpenseInput]) -> Result<Vec<ParsedLine>, AppError> {
    if inputs.len() > 40 {
        return Err(AppError::BadRequest("Keep the expenses to 40 lines.".into()));
    }
    let mut lines = Vec::new();
    for input in inputs {
        let description = input.description.trim();
        let category = input.category.trim();
        if description.is_empty() {
            return Err(AppError::BadRequest("Enter a description for each expense.".into()));
        }
        if description.chars().count() > 80 {
            return Err(AppError::BadRequest("An expense description is too long.".into()));
        }
        if !CATEGORIES.contains(&category) {
            return Err(AppError::BadRequest("Choose an expense category.".into()));
        }
        let amount = whole(input.amount, "Expense amount")?;
        lines.push(ParsedLine {
            description: description.to_string(),
            category: category.to_string(),
            amount,
        });
    }
    Ok(lines)
}

fn whole(value: i64, label: &str) -> Result<i32, AppError> {
    if !(0..=1_000_000_000).contains(&value) {
        return Err(AppError::BadRequest(format!("{label} is a whole amount.")));
    }
    Ok(value as i32)
}
