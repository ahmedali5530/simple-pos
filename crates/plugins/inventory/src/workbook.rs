use std::collections::HashMap;

use async_trait::async_trait;
use catalog_contract::CatalogApi;
use inventory_contract::{
    InventoryWorkbook, MovementExportRow, MovementImportDraft, MovementImportResult,
};
use platform_kernel::context::TenantTx;
use platform_kernel::error::AppError;

pub struct PgInventoryWorkbook {
    pub catalog: CatalogApi,
}

#[async_trait]
impl InventoryWorkbook for PgInventoryWorkbook {
    async fn movement_rows(
        &self,
        tx: &mut TenantTx<'_>,
    ) -> Result<Vec<MovementExportRow>, AppError> {
        let snapshot = self.catalog.0.snapshot(tx).await?;
        let items: HashMap<i32, &str> = snapshot
            .items
            .iter()
            .map(|item| (item.id, item.name.as_str()))
            .collect();
        let barcodes: HashMap<i32, &str> = snapshot
            .items
            .iter()
            .filter_map(|item| item.barcode.as_deref().map(|barcode| (item.id, barcode)))
            .collect();
        let item_sections: HashMap<i32, i32> = snapshot
            .items
            .iter()
            .map(|item| (item.id, item.section_id))
            .collect();
        let sections: HashMap<i32, &str> = snapshot
            .sections
            .iter()
            .map(|section| (section.id, section.short_name.as_str()))
            .collect();
        let rows = sqlx::query_as::<_, (i32, String, i32, Option<String>, i32, String)>(
            r#"
            SELECT id,
                   to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD HH24:MI'),
                   item_id, bill_no, qty, reason
            FROM stock_movements
            ORDER BY created_at DESC, id DESC
            LIMIT 5000
            "#,
        )
        .fetch_all(tx.conn())
        .await?;
        Ok(rows
            .into_iter()
            .map(
                |(id, time_utc, item_id, bill_no, qty, reason)| MovementExportRow {
                    id,
                    time_utc,
                    barcode: barcodes.get(&item_id).map(|barcode| barcode.to_string()),
                    item: items
                        .get(&item_id)
                        .copied()
                        .unwrap_or("Unknown item")
                        .to_string(),
                    section: item_sections
                        .get(&item_id)
                        .and_then(|section_id| sections.get(section_id))
                        .copied()
                        .unwrap_or("")
                        .to_string(),
                    qty,
                    reason,
                    bill_no,
                },
            )
            .collect())
    }

    async fn apply_movements(
        &self,
        tx: &mut TenantTx<'_>,
        drafts: &[MovementImportDraft],
    ) -> Result<MovementImportResult, AppError> {
        if drafts.is_empty() {
            return Ok(MovementImportResult { applied: 0 });
        }
        let snapshot = self.catalog.0.snapshot(tx).await?;
        let mut by_barcode: HashMap<&str, (i32, &str)> = HashMap::new();
        let mut by_name: HashMap<String, (i32, &str)> = HashMap::new();
        for item in &snapshot.items {
            if let Some(barcode) = item.barcode.as_deref() {
                by_barcode.insert(barcode, (item.id, item.name.as_str()));
            }
            by_name.insert(
                item.name.to_ascii_lowercase(),
                (item.id, item.name.as_str()),
            );
            for variant in &item.variants {
                if let Some(barcode) = variant.barcode.as_deref() {
                    by_barcode.insert(barcode, (item.id, item.name.as_str()));
                }
            }
        }
        let mut resolved = Vec::with_capacity(drafts.len());
        for draft in drafts {
            let found = draft
                .barcode
                .as_deref()
                .and_then(|code| by_barcode.get(code).copied())
                .or_else(|| {
                    let name = draft.item_name.trim().to_ascii_lowercase();
                    (!name.is_empty())
                        .then(|| by_name.get(&name).copied())
                        .flatten()
                })
                .ok_or_else(|| {
                    AppError::BadRequest(format!(
                        "Movements row {}: that item is not in the catalog.",
                        draft.row
                    ))
                })?;
            resolved.push((draft, found));
        }
        let mut ids: Vec<i32> = resolved.iter().map(|(_, (id, _))| *id).collect();
        ids.sort_unstable();
        ids.dedup();
        for id in &ids {
            sqlx::query(
                r#"
                INSERT INTO inventory_balances (tenant_id, item_id, stock, avg_cost)
                VALUES (current_setting('app.tenant_id')::uuid, $1, 0, 0)
                ON CONFLICT (tenant_id, item_id) DO NOTHING
                "#,
            )
            .bind(id)
            .execute(tx.conn())
            .await?;
        }
        let locked = sqlx::query_as::<_, (i32, i32)>(
            "SELECT item_id, stock FROM inventory_balances WHERE item_id = ANY($1) ORDER BY item_id FOR UPDATE",
        )
        .bind(&ids)
        .fetch_all(tx.conn())
        .await?;
        if locked.len() != ids.len() {
            return Err(AppError::BadRequest(
                "One of those items is no longer in the catalog.".into(),
            ));
        }
        let mut on_hand: HashMap<i32, i32> = locked.into_iter().collect();
        for (draft, (id, name)) in &resolved {
            let stock = on_hand.get_mut(id).expect("locked");
            let next = i64::from(*stock) + i64::from(draft.qty);
            if next < 0 {
                return Err(AppError::BadRequest(format!(
                    "Movements row {}: only {stock} of {name} are on hand.",
                    draft.row
                )));
            }
            if next > i64::from(i32::MAX) {
                return Err(AppError::BadRequest(format!(
                    "Movements row {}: that quantity is too large.",
                    draft.row
                )));
            }
            *stock = next as i32;
        }
        for (draft, (id, _)) in &resolved {
            sqlx::query("UPDATE inventory_balances SET stock = stock + $1 WHERE item_id = $2")
                .bind(draft.qty)
                .bind(id)
                .execute(tx.conn())
                .await?;
            sqlx::query("INSERT INTO stock_movements (item_id, qty, reason) VALUES ($1, $2, $3)")
                .bind(id)
                .bind(draft.qty)
                .bind(movement_reason(draft.qty > 0, &draft.note))
                .execute(tx.conn())
                .await?;
        }
        Ok(MovementImportResult {
            applied: drafts.len() as i64,
        })
    }
    async fn apply_stock_delta(
        &self,
        tx: &mut TenantTx<'_>,
        item_id: i32,
        delta: i32,
        note: &str,
    ) -> Result<(), AppError> {
        if delta == 0 {
            return Ok(());
        }
        sqlx::query(
            r#"
            INSERT INTO inventory_balances (tenant_id, item_id, stock, avg_cost)
            VALUES (current_setting('app.tenant_id')::uuid, $1, 0, 0)
            ON CONFLICT (tenant_id, item_id) DO NOTHING
            "#,
        )
        .bind(item_id)
        .execute(tx.conn())
        .await?;
        let updated = sqlx::query(
            "UPDATE inventory_balances SET stock = stock + $1 WHERE item_id = $2 AND stock + $1 >= 0",
        )
        .bind(delta)
        .bind(item_id)
        .execute(tx.conn())
        .await?;
        if updated.rows_affected() == 0 {
            return Err(AppError::BadRequest(
                "That stock count would take the item below zero.".into(),
            ));
        }
        sqlx::query("INSERT INTO stock_movements (item_id, qty, reason) VALUES ($1, $2, $3)")
            .bind(item_id)
            .bind(delta)
            .bind(movement_reason(delta > 0, note))
            .execute(tx.conn())
            .await?;
        Ok(())
    }
}

fn movement_reason(inbound: bool, note: &str) -> String {
    let label = if inbound { "Received" } else { "Removed" };
    let note = note.trim();
    if note.is_empty() || note.eq_ignore_ascii_case(label) {
        label.to_string()
    } else if note.starts_with(&format!("{label} · ")) {
        note.to_string()
    } else {
        format!("{label} · {note}")
    }
}
