use async_trait::async_trait;
use platform_kernel::context::TenantTx;
use platform_kernel::error::AppError;
use sales_contract::{SaleTender, TenderApply, TenderApplyContext};

pub struct GiftTenderApply;

#[async_trait]
impl TenderApply for GiftTenderApply {
    async fn apply(
        &self,
        tx: &mut TenantTx<'_>,
        ctx: &TenderApplyContext<'_>,
        tender: &SaleTender,
    ) -> Result<bool, AppError> {
        if tender.method != "gift.card" {
            return Ok(false);
        }
        let code = normalize_code(&tender.reference)?;
        let row = sqlx::query_as::<_, CardRow>(
            r#"
            SELECT id, balance, status
            FROM gift_cards
            WHERE code = $1
            FOR UPDATE
            "#,
        )
        .bind(&code)
        .fetch_optional(tx.conn())
        .await?
        .ok_or_else(|| AppError::BadRequest(format!("Gift card {code} was not found.")))?;
        if row.status != "active" {
            return Err(AppError::BadRequest(format!("Gift card {code} is void.")));
        }
        if tender.amount > row.balance {
            return Err(AppError::BadRequest(format!(
                "Gift card {code} only has {} left.",
                row.balance
            )));
        }
        let balance_after = row.balance - tender.amount;
        sqlx::query(
            r#"
            UPDATE gift_cards
            SET balance = $2, updated_at = now()
            WHERE id = $1
            "#,
        )
        .bind(row.id)
        .bind(balance_after)
        .execute(tx.conn())
        .await?;
        sqlx::query(
            r#"
            INSERT INTO gift_ledger
                (card_id, kind, amount, balance_after, bill_id, bill_no, note)
            VALUES ($1, 'redeem', $2, $3, $4, $5, '')
            "#,
        )
        .bind(row.id)
        .bind(-tender.amount)
        .bind(balance_after)
        .bind(ctx.bill_id)
        .bind(ctx.bill_no)
        .execute(tx.conn())
        .await?;
        Ok(true)
    }
}

#[derive(sqlx::FromRow)]
struct CardRow {
    id: i32,
    balance: i32,
    status: String,
}

pub fn normalize_code(raw: &str) -> Result<String, AppError> {
    let code = raw
        .trim()
        .trim_start_matches("card:")
        .trim()
        .to_ascii_uppercase();
    if code.is_empty() {
        return Err(AppError::BadRequest(
            "Enter the gift card code before redeeming.".into(),
        ));
    }
    if code.chars().count() > 64 {
        return Err(AppError::BadRequest("That gift card code is too long.".into()));
    }
    Ok(code)
}
