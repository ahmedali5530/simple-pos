use axum::extract::Query;
use axum::response::Response;
use axum::Json;
use platform_kernel::error::AppError;
use platform_kernel::routes::{PluginRoutes, RegistrationError, TenantDb};
use serde::{Deserialize, Serialize};

use crate::export::{self, ExportFormat};
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

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct LiabilityRow {
    pub id: i32,
    pub code: String,
    pub issued: i32,
    pub balance: i32,
    pub status: String,
    pub issued_bill_no: String,
    pub issued_at: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ExportQuery {
    format: String,
}

#[derive(Debug, Serialize)]
struct LiabilityReport {
    cards: i64,
    /// Sum of original issue amounts on active cards with a remaining balance.
    issued: i64,
    /// Sum of remaining balances (outstanding liability).
    outstanding: i64,
    rows: Vec<LiabilityRow>,
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

#[derive(Debug, Deserialize)]
struct IssueBody {
    code: String,
    amount: i32,
}

pub fn mount(routes: &mut PluginRoutes) -> Result<(), RegistrationError> {
    routes.get("cards", list_cards, "gift.read")?;
    routes.get("balance", balance, "gift.read")?;
    routes.get("liability", liability, "gift.read")?;
    routes.get("export", export_report, "gift.read")?;
    routes.post("issue", issue_card, "gift.write")?;
    routes.post("void", void_card, "gift.write")?;
    Ok(())
}

async fn export_report(
    db: TenantDb,
    Query(query): Query<ExportQuery>,
) -> Result<Response, AppError> {
    let format = ExportFormat::parse(&query.format)?;
    let report = load_liability(&db).await?;
    let bytes = export::build(
        format,
        report.cards,
        report.issued,
        report.outstanding,
        &report.rows,
    )?;
    Ok(export::file_response(
        &format!("gift-liability.{}", format.extension()),
        format.content_type(),
        bytes,
    ))
}

async fn issue_card(db: TenantDb, Json(body): Json<IssueBody>) -> Result<Json<CardView>, AppError> {
    let code = normalize_code(&body.code)?;
    if body.amount < 1 {
        return Err(AppError::BadRequest(
            "Enter a gift card amount greater than zero.".into(),
        ));
    }
    let mut tx = db.begin(false).await?;
    let existing: Option<(i32, String)> =
        sqlx::query_as("SELECT id, status FROM gift_cards WHERE code = $1")
            .bind(&code)
            .fetch_optional(tx.conn())
            .await?;
    if let Some((_, status)) = existing {
        return Err(AppError::BadRequest(if status == "void" {
            format!("Gift card {code} is void and cannot be reissued.")
        } else {
            format!("Gift card {code} already exists.")
        }));
    }
    let id: i32 = sqlx::query_scalar(
        r#"
        INSERT INTO gift_cards
            (code, balance, status, issued_bill_id, issued_bill_no, issued_at)
        VALUES ($1, $2, 'active', NULL, '', now())
        RETURNING id
        "#,
    )
    .bind(&code)
    .bind(body.amount)
    .fetch_one(tx.conn())
    .await?;
    sqlx::query(
        r#"
        INSERT INTO gift_ledger
            (card_id, kind, amount, balance_after, bill_id, bill_no, note)
        VALUES ($1, 'issue', $2, $2, NULL, '', 'issued in settings')
        "#,
    )
    .bind(id)
    .bind(body.amount)
    .execute(tx.conn())
    .await?;
    let row = sqlx::query_as::<_, CardView>(
        r#"
        SELECT id, code, balance, status, issued_bill_no,
               issued_at::text AS issued_at
        FROM gift_cards
        WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_one(tx.conn())
    .await?;
    tx.commit().await?;
    Ok(Json(row))
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

async fn load_liability(db: &TenantDb) -> Result<LiabilityReport, AppError> {
    let mut tx = db.begin(true).await?;
    let rows = sqlx::query_as::<_, LiabilityRow>(
        r#"
        SELECT
            c.id,
            c.code,
            COALESCE((
                SELECT SUM(l.amount)::int
                FROM gift_ledger l
                WHERE l.card_id = c.id AND l.kind = 'issue'
            ), c.balance) AS issued,
            c.balance,
            c.status,
            c.issued_bill_no,
            c.issued_at::text AS issued_at
        FROM gift_cards c
        WHERE c.status = 'active' AND c.balance > 0
        ORDER BY c.balance DESC, c.code
        "#,
    )
    .fetch_all(tx.conn())
    .await?;
    let cards = rows.len() as i64;
    let issued = rows.iter().map(|row| i64::from(row.issued)).sum();
    let outstanding = rows.iter().map(|row| i64::from(row.balance)).sum();
    tx.commit().await?;
    Ok(LiabilityReport {
        cards,
        issued,
        outstanding,
        rows,
    })
}

async fn liability(db: TenantDb) -> Result<Json<LiabilityReport>, AppError> {
    Ok(Json(load_liability(&db).await?))
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
