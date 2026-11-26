use axum::Json;
use platform_kernel::error::AppError;
use platform_kernel::routes::{PluginRoutes, RegistrationError, TenantDb};
use serde::{Deserialize, Serialize};

use crate::tender::normalize_code;

#[derive(Debug, Serialize, sqlx::FromRow)]
struct CardView {
    id: i32,
    code: String,
    balance: i32,
    status: String,
    issued_bill_no: String,
    issued_at: Option<String>,
}

#[derive(Debug, Serialize)]
struct LiabilityReport {
    cards: i64,
    outstanding: i64,
    rows: Vec<CardView>,
}

#[derive(Debug, Deserialize)]
struct LookupQuery {
    code: String,
}

#[derive(Debug, Serialize)]
struct BalanceView {
    code: String,
    balance: i32,
    status: String,
}

#[derive(Debug, Deserialize)]
struct VoidBody {
    code: String,
}

pub fn mount(routes: &mut PluginRoutes) -> Result<(), RegistrationError> {
    routes.get("cards", list_cards, "gift.read")?;
    routes.get("balance", balance, "gift.read")?;
    routes.get("liability", liability, "gift.read")?;
    routes.post("void", void_card, "gift.write")?;
    Ok(())
}

async fn list_cards(db: TenantDb) -> Result<Json<Vec<CardView>>, AppError> {
    let mut tx = db.begin(true).await?;
    let rows = sqlx::query_as::<_, CardView>(
        r#"
        SELECT id, code, balance, status, issued_bill_no,
               issued_at::text AS issued_at
        FROM gift_cards
        ORDER BY created_at DESC, id DESC
        LIMIT 200
        "#,
    )
    .fetch_all(tx.conn())
    .await?;
    tx.commit().await?;
    Ok(Json(rows))
}

async fn balance(
    db: TenantDb,
    axum::extract::Query(query): axum::extract::Query<LookupQuery>,
) -> Result<Json<BalanceView>, AppError> {
    let code = normalize_code(&query.code)?;
    let mut tx = db.begin(true).await?;
    let row = sqlx::query_as::<_, (String, i32, String)>(
        "SELECT code, balance, status FROM gift_cards WHERE code = $1",
    )
    .bind(&code)
    .fetch_optional(tx.conn())
    .await?
    .ok_or_else(|| AppError::NotFound(format!("Gift card {code} was not found.")))?;
    tx.commit().await?;
    Ok(Json(BalanceView {
        code: row.0,
        balance: row.1,
        status: row.2,
    }))
}

async fn liability(db: TenantDb) -> Result<Json<LiabilityReport>, AppError> {
    let mut tx = db.begin(true).await?;
    let rows = sqlx::query_as::<_, CardView>(
        r#"
        SELECT id, code, balance, status, issued_bill_no,
               issued_at::text AS issued_at
        FROM gift_cards
        WHERE status = 'active' AND balance > 0
        ORDER BY balance DESC, code
        "#,
    )
    .fetch_all(tx.conn())
    .await?;
    let cards = rows.len() as i64;
    let outstanding = rows.iter().map(|row| i64::from(row.balance)).sum();
    tx.commit().await?;
    Ok(Json(LiabilityReport {
        cards,
        outstanding,
        rows,
    }))
}

async fn void_card(db: TenantDb, Json(body): Json<VoidBody>) -> Result<Json<CardView>, AppError> {
    let code = normalize_code(&body.code)?;
    let mut tx = db.begin(false).await?;
    let row = sqlx::query_as::<_, CardView>(
        r#"
        SELECT id, code, balance, status, issued_bill_no,
               issued_at::text AS issued_at
        FROM gift_cards
        WHERE code = $1
        FOR UPDATE
        "#,
    )
    .bind(&code)
    .fetch_optional(tx.conn())
    .await?
    .ok_or_else(|| AppError::NotFound(format!("Gift card {code} was not found.")))?;
    if row.status == "void" {
        tx.commit().await?;
        return Ok(Json(row));
    }
    sqlx::query(
        r#"
        UPDATE gift_cards
        SET status = 'void', balance = 0, updated_at = now()
        WHERE id = $1
        "#,
    )
    .bind(row.id)
    .execute(tx.conn())
    .await?;
    sqlx::query(
        r#"
        INSERT INTO gift_ledger
            (card_id, kind, amount, balance_after, bill_id, bill_no, note)
        VALUES ($1, 'void', $2, 0, NULL, '', 'voided in settings')
        "#,
    )
    .bind(row.id)
    .bind(-row.balance)
    .execute(tx.conn())
    .await?;
    let updated = CardView {
        status: "void".into(),
        balance: 0,
        ..row
    };
    tx.commit().await?;
    Ok(Json(updated))
}
