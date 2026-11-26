use crate::barcode::{codes_match, embedded_sale, parse_embedded, prompt_sale};
use crate::models::{
    BarcodeRule, BillLineInput, Category, ItemVariant, PosItem, PosSection, PosSnapshot,
    PostedBill, SectionChoice, SectionLayout, TenderInput, TillShop,
};
use platform_kernel::error::AppError;
use sales_contract::{BillPostedV4, SaleLine, SaleTender};

struct Sale {
    item_id: i32,
    variant_id: Option<i32>,
    line_name: String,
    rate: i32,
    qty: i32,
    units: i32,
    amount: i32,
    unit_cost: i64,
    cost_total: i64,
    source: String,
    reference: String,
    details: serde_json::Value,
}

const DEFAULT_LINE_SOURCE: &str = "catalog.product";
const MAX_SOURCE_LEN: usize = 128;
const MAX_REFERENCE_LEN: usize = 512;
const MAX_DETAILS_BYTES: usize = 16 * 1024;

/// A posted bill plus the event body, so the route can publish after the sale commits.
pub struct PostBillOutcome {
    pub posted: PostedBill,
    pub event: BillPostedV4,
}

const DEFAULT_TENDER_METHOD: &str = "cash";
const MAX_TENDER_METHOD_LEN: usize = 64;
const MAX_TENDER_REFERENCE_LEN: usize = 512;
const MAX_TENDER_DETAILS_BYTES: usize = 16 * 1024;

fn normalize_tenders(raw: &[TenderInput], total: i32) -> Result<Vec<SaleTender>, AppError> {
    if raw.is_empty() {
        return Ok(vec![SaleTender {
            method: DEFAULT_TENDER_METHOD.into(),
            amount: total,
            reference: String::new(),
            details: serde_json::json!({}),
        }]);
    }
    let mut tenders = Vec::with_capacity(raw.len());
    let mut sum = 0_i32;
    for tender in raw {
        let method = tender.method.trim();
        if method.is_empty() {
            return Err(AppError::BadRequest("Every tender needs a method.".into()));
        }
        if method.chars().count() > MAX_TENDER_METHOD_LEN {
            return Err(AppError::BadRequest("That tender method is too long.".into()));
        }
        if tender.amount < 1 {
            return Err(AppError::BadRequest(
                "Every tender amount has to be at least 1.".into(),
            ));
        }
        let reference = tender
            .reference
            .as_deref()
            .map(str::trim)
            .unwrap_or("")
            .to_string();
        if reference.chars().count() > MAX_TENDER_REFERENCE_LEN {
            return Err(AppError::BadRequest(
                "That tender reference is too long.".into(),
            ));
        }
        let details = match tender.details.as_ref() {
            None | Some(serde_json::Value::Null) => serde_json::json!({}),
            Some(value) => {
                let size = serde_json::to_vec(value)
                    .map_err(|err| {
                        AppError::BadRequest(format!("That tender detail is not valid JSON: {err}"))
                    })?
                    .len();
                if size > MAX_TENDER_DETAILS_BYTES {
                    return Err(AppError::BadRequest(
                        "That tender detail is too large.".into(),
                    ));
                }
                value.clone()
            }
        };
        sum = sum
            .checked_add(tender.amount)
            .ok_or_else(|| AppError::BadRequest("Those tender amounts are too large.".into()))?;
        tenders.push(SaleTender {
            method: method.to_string(),
            amount: tender.amount,
            reference,
            details,
        });
    }
    if sum != total {
        return Err(AppError::BadRequest(format!(
            "Tenders must add up to the bill total ({total})."
        )));
    }
    Ok(tenders)
}

fn normalize_line_metadata(
    line: &BillLineInput,
) -> Result<(String, String, serde_json::Value), AppError> {
    let source = line
        .source
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(DEFAULT_LINE_SOURCE);
    if source.chars().count() > MAX_SOURCE_LEN {
        return Err(AppError::BadRequest("That line source is too long.".into()));
    }
    let reference = line.reference.as_deref().map(str::trim).unwrap_or("");
    if reference.chars().count() > MAX_REFERENCE_LEN {
        return Err(AppError::BadRequest(
            "That line reference is too long.".into(),
        ));
    }
    let details = match line.details.as_ref() {
        None | Some(serde_json::Value::Null) => serde_json::json!({}),
        Some(value) => {
            let size = serde_json::to_vec(value)
                .map_err(|err| {
                    AppError::BadRequest(format!("That line detail is not valid JSON: {err}"))
                })?
                .len();
            if size > MAX_DETAILS_BYTES {
                return Err(AppError::BadRequest(
                    "That line detail is too large.".into(),
                ));
            }
            value.clone()
        }
    };
    Ok((source.to_string(), reference.to_string(), details))
}

fn event_lines(prepared: &[Sale]) -> Vec<SaleLine> {
    prepared
        .iter()
        .map(|sale| SaleLine {
            source: sale.source.clone(),
            reference: sale.reference.clone(),
            label: sale.line_name.clone(),
            quantity: sale.qty.to_string(),
            unit_amount: i64::from(sale.rate),
            amount: i64::from(sale.amount),
            item_id: Some(sale.item_id),
            stock_quantity: if sale.reference.is_empty() {
                sale.qty * sale.units
            } else {
                0
            },
            details: sale.details.clone(),
        })
        .collect()
}

#[derive(sqlx::FromRow)]
struct ShopRow {
    name: String,
    currency: String,
    locale: String,
    bill_prefix: String,
}

#[derive(sqlx::FromRow)]
struct RuleRow {
    prefix: String,
    mode: String,
    scale: i32,
}

pub async fn current_shop_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    fallback: &TillShop,
) -> Result<TillShop, AppError> {
    let row = sqlx::query_as::<_, ShopRow>(
        r#"
        SELECT profile.name, profile.currency, profile.locale,
               COALESCE(settings.bill_prefix, 'BILL') AS bill_prefix
        FROM tenant_profile profile
        LEFT JOIN pos_settings settings ON settings.tenant_id = profile.tenant_id
        WHERE profile.tenant_id = current_setting('app.tenant_id')::uuid
        "#,
    )
    .fetch_optional(&mut **tx)
    .await?;
    let Some(row) = row else {
        return Ok(fallback.clone());
    };
    let rules = sqlx::query_as::<_, RuleRow>(
        "SELECT prefix, mode, scale FROM barcode_rules_v2 ORDER BY sort_order, id",
    )
    .fetch_all(&mut **tx)
    .await?;
    Ok(TillShop {
        name: row.name,
        currency: row.currency,
        locale: row.locale,
        bill_prefix: row.bill_prefix,
        barcode_rules: rules
            .into_iter()
            .map(|rule| BarcodeRule {
                prefix: rule.prefix,
                mode: rule.mode,
                scale: rule.scale,
            })
            .collect(),
    })
}

/// Shop settings and empty till layout when no catalog provider is registered.
pub async fn load_pos_shop_tx(
    tx: &mut platform_kernel::context::TenantTx<'_>,
    shop: &TillShop,
) -> Result<PosSnapshot, AppError> {
    let shop = current_shop_tx(tx.transaction(), shop).await?;
    let last_section_id =
        sqlx::query_scalar::<_, Option<i32>>("SELECT last_section_id FROM pos_state WHERE id = 1")
            .fetch_optional(tx.conn())
            .await?
            .flatten();
    Ok(PosSnapshot {
        shop_name: shop.name.clone(),
        bill_prefix: shop.bill_prefix.clone(),
        currency: shop.currency.clone(),
        locale: shop.locale.clone(),
        barcode_rules: shop.barcode_rules.clone(),
        sections: Vec::new(),
        all_sections: Vec::new(),
        categories: Vec::new(),
        last_section_id,
    })
}

/// Load the till snapshot through the catalog contract, so POS does not read
/// catalog tables for the main till view.
pub async fn load_pos_catalog_tx(
    tx: &mut platform_kernel::context::TenantTx<'_>,
    shop: &TillShop,
    catalog: &catalog_contract::CatalogApi,
) -> Result<PosSnapshot, AppError> {
    let shop = current_shop_tx(tx.transaction(), shop).await?;
    let snapshot = catalog.0.snapshot(tx).await?;
    let all_sections = snapshot
        .sections
        .iter()
        .map(|section| SectionChoice {
            id: section.id,
            name: section.name.clone(),
            short_name: section.short_name.clone(),
            kind: section.kind.clone(),
            accent: section.accent.clone(),
            show_on_pos: section.show_on_pos,
            pos_order: section.pos_order,
        })
        .collect::<Vec<_>>();
    let categories = snapshot
        .categories
        .iter()
        .map(|category| Category {
            id: category.id,
            name: category.name.clone(),
            sort_order: category.sort_order,
        })
        .collect::<Vec<_>>();
    let items = snapshot
        .items
        .iter()
        .map(|item| PosItem {
            id: item.id,
            section_id: item.section_id,
            category_id: item.category_id,
            category_name: item.category_name.clone(),
            barcode: item.barcode.clone(),
            name: item.name.clone(),
            rate: item.rate,
            cost: item.cost,
            stock: item.stock,
            portion: item.portion.clone(),
            group_name: item.group_name.clone(),
            signature: item.signature,
            prompt: item.prompt.clone(),
            base_rate: item.base_rate,
            scale: item.scale,
            image_url: item.image_url.clone(),
            variants: item
                .variants
                .iter()
                .map(|variant| ItemVariant {
                    id: variant.id,
                    item_id: variant.item_id,
                    name: variant.name.clone(),
                    rate: variant.rate,
                    units: variant.units,
                    barcode: variant.barcode.clone(),
                })
                .collect(),
        })
        .collect::<Vec<_>>();
    let last_section_id =
        sqlx::query_scalar::<_, Option<i32>>("SELECT last_section_id FROM pos_state WHERE id = 1")
            .fetch_optional(tx.conn())
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
        bill_prefix: shop.bill_prefix.clone(),
        currency: shop.currency.clone(),
        locale: shop.locale.clone(),
        barcode_rules: shop.barcode_rules.clone(),
        sections,
        all_sections,
        categories,
        last_section_id,
    })
}

pub async fn set_pos_focus_tx(
    tx: &mut platform_kernel::context::TenantTx<'_>,
    catalog: &catalog_contract::CatalogApi,
    section_id: i32,
) -> Result<i32, AppError> {
    let snapshot = catalog.0.snapshot(tx).await?;
    if !snapshot
        .sections
        .iter()
        .any(|section| section.id == section_id)
    {
        return Err(AppError::NotFound(
            "That section is no longer in the catalog.".into(),
        ));
    }
    sqlx::query("UPDATE pos_state SET last_section_id = $1 WHERE id = 1")
        .bind(section_id)
        .execute(tx.conn())
        .await?;
    Ok(section_id)
}

pub async fn set_pos_sections_tx(
    tx: &mut platform_kernel::context::TenantTx<'_>,
    shop: &TillShop,
    catalog: &catalog_contract::CatalogApi,
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
    let snapshot = catalog.0.snapshot(tx).await?;
    let known = snapshot
        .sections
        .iter()
        .filter(|section| layout.iter().any(|entry| entry.id == section.id))
        .count();
    if known != layout.len() {
        return Err(AppError::NotFound(
            "One of those sections is no longer in the catalog.".into(),
        ));
    }
    catalog
        .0
        .set_pos_layout(
            tx,
            &layout
                .iter()
                .enumerate()
                .map(|(index, section)| (section.id, section.show_on_pos, index as i32))
                .collect::<Vec<_>>(),
        )
        .await?;
    load_pos_catalog_tx(tx, shop, catalog).await
}

fn map_bill_line_error(err: sqlx::Error) -> AppError {
    let Some(db) = err.as_database_error() else {
        return AppError::Db(err);
    };
    if db.code().as_deref() == Some("P0001") {
        return AppError::BadRequest(db.message().to_string());
    }
    AppError::Db(err)
}

pub async fn post_bill_tx(
    tx: &mut platform_kernel::context::TenantTx<'_>,
    shop: &TillShop,
    catalog: &catalog_contract::CatalogApi,
    user_id: i32,
    section_id: i32,
    lines: &[BillLineInput],
    customer: Option<&str>,
    tenders: &[TenderInput],
) -> Result<PostBillOutcome, AppError> {
    if lines.is_empty() {
        return Err(AppError::BadRequest(
            "Add at least one item before posting the bill.".into(),
        ));
    }
    let customer = customer.map(str::trim).filter(|name| !name.is_empty());
    if customer.is_some_and(|name| name.chars().count() > 80) {
        return Err(AppError::BadRequest(
            "Keep the customer name to 80 characters.".into(),
        ));
    }
    let shop = current_shop_tx(tx.transaction(), shop).await?;
    let shop = &shop;
    let snapshot = catalog.0.snapshot(tx).await?;
    let section_name = snapshot
        .sections
        .iter()
        .find(|section| section.id == section_id)
        .map(|section| section.name.clone())
        .ok_or_else(|| AppError::NotFound("That section is not in the catalog.".into()))?;
    let items: std::collections::HashMap<i32, catalog_contract::CatalogItem> = snapshot
        .items
        .into_iter()
        .map(|item| (item.id, item))
        .collect();

    let mut total = 0_i32;
    let mut prepared: Vec<Sale> = Vec::with_capacity(lines.len());
    let mut taken: std::collections::HashMap<i32, i32> = std::collections::HashMap::new();

    for line in lines {
        if line.qty < 1 {
            return Err(AppError::BadRequest(
                "Quantity has to be at least 1.".into(),
            ));
        }
        if line.rate < 1 {
            return Err(AppError::BadRequest("Rate has to be at least 1.".into()));
        }
        let (source, reference, details) = normalize_line_metadata(line)?;
        let item = items
            .get(&line.item_id)
            .filter(|item| item.section_id == section_id)
            .cloned()
            .ok_or_else(|| {
                AppError::BadRequest(format!("That item is not sold in {section_name}."))
            })?;

        let (variant_id, units, line_name, catalog_rate, stored_barcode) = match line.variant_id {
            Some(variant_id) => {
                let variant = item
                    .variants
                    .iter()
                    .find(|variant| variant.id == variant_id)
                    .ok_or_else(|| {
                        AppError::BadRequest(format!("{} does not have that option.", item.name))
                    })?;
                (
                    Some(variant.id),
                    variant.units,
                    format!("{} · {}", item.name, variant.name),
                    variant.rate,
                    variant.barcode.clone().or(item.barcode.clone()),
                )
            }
            None => {
                if !item.variants.is_empty() {
                    return Err(AppError::BadRequest(format!(
                        "Choose an option for {}.",
                        item.name
                    )));
                }
                (None, 1, item.name.clone(), item.rate, item.barcode.clone())
            }
        };
        let (rate, units, line_name) =
            if let Some(code) = line.code.as_deref().filter(|code| !code.is_empty()) {
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
                let (rate, stock_each, label) =
                    embedded_sale(catalog_rate, &embedded).map_err(AppError::BadRequest)?;
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
                let quote = prompt_sale(&item.prompt, base_rate, item.scale, entry)
                    .map_err(AppError::BadRequest)?;
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
        let stock_units = units
            .checked_mul(line.qty)
            .ok_or_else(|| AppError::BadRequest("That line takes too much stock.".into()))?;
        // A referenced line is one external unit (an IMEI, a job). Its source
        // owns availability, so it does not consume the catalog quantity.
        if reference.is_empty() {
            let used = taken.entry(item.id).or_insert(0);
            *used = used
                .checked_add(stock_units)
                .ok_or_else(|| AppError::BadRequest("That bill takes too much stock.".into()))?;
            if *used > item.stock {
                return Err(AppError::BadRequest(format!(
                    "Only {} left for {}.",
                    item.stock, item.name
                )));
            }
        }
        let amount = line
            .rate
            .checked_mul(line.qty)
            .ok_or_else(|| AppError::BadRequest("That line amount is too large.".into()))?;
        let unit_cost = i64::from(units)
            .checked_mul(item.cost)
            .ok_or_else(|| AppError::BadRequest("That line cost is too large.".into()))?;
        let cost_total = unit_cost
            .checked_mul(i64::from(line.qty))
            .ok_or_else(|| AppError::BadRequest("That line cost is too large.".into()))?;
        total = total
            .checked_add(amount)
            .ok_or_else(|| AppError::BadRequest("That bill total is too large.".into()))?;
        prepared.push(Sale {
            item_id: item.id,
            variant_id,
            line_name,
            rate: line.rate,
            qty: line.qty,
            units,
            amount,
            unit_cost,
            cost_total,
            source,
            reference,
            details,
        });
    }

    let bill_id = sqlx::query_scalar::<_, i32>("SELECT nextval('bills_id_seq')::int")
        .fetch_one(tx.conn())
        .await?;
    let bill_no = format!("{}-{}", shop.bill_prefix, 1000 + bill_id);
    sqlx::query(
        r#"
        INSERT INTO bills (id, bill_no, section_id, user_id, total, customer)
        VALUES ($1, $2, $3, $4, $5, $6)
        "#,
    )
    .bind(bill_id)
    .bind(&bill_no)
    .bind(section_id)
    .bind(user_id)
    .bind(total)
    .bind(customer)
    .execute(tx.conn())
    .await?;

    for sale in &prepared {
        sqlx::query(
            r#"
            INSERT INTO bill_lines
                (bill_id, item_id, variant_id, name, rate, qty, units, amount, unit_cost, cost_total, source, reference, details)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
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
        .bind(sale.unit_cost)
        .bind(sale.cost_total)
        .bind(&sale.source)
        .bind(&sale.reference)
        .bind(&sale.details)
        .execute(tx.conn())
        .await
        .map_err(map_bill_line_error)?;
    }

    let tenders = normalize_tenders(tenders, total)?;
    for tender in &tenders {
        sqlx::query(
            r#"
            INSERT INTO bill_tenders (bill_id, method, amount, reference, details)
            VALUES ($1, $2, $3, $4, $5)
            "#,
        )
        .bind(bill_id)
        .bind(&tender.method)
        .bind(tender.amount)
        .bind(&tender.reference)
        .bind(&tender.details)
        .execute(tx.conn())
        .await?;
    }

    let posted_at: String = sqlx::query_scalar("SELECT created_at::text FROM bills WHERE id = $1")
        .bind(bill_id)
        .fetch_one(tx.conn())
        .await?;
    let event = BillPostedV4 {
        bill_id,
        bill_no: bill_no.clone(),
        section_id,
        user_id,
        total,
        customer: customer.map(str::to_string),
        posted_at,
        lines: event_lines(&prepared),
        tenders,
    };
    let pos = load_pos_catalog_tx(tx, shop, catalog).await?;
    Ok(PostBillOutcome {
        posted: PostedBill {
            bill_no,
            total,
            pos,
        },
        event,
    })
}

#[cfg(test)]
mod line_metadata_tests {
    use super::*;

    fn input(
        source: Option<&str>,
        reference: Option<&str>,
        details: Option<serde_json::Value>,
    ) -> BillLineInput {
        BillLineInput {
            item_id: 1,
            variant_id: None,
            rate: 100,
            qty: 1,
            code: None,
            entry: None,
            source: source.map(str::to_string),
            reference: reference.map(str::to_string),
            details,
        }
    }

    #[test]
    fn defaults_to_catalog_product() {
        let (source, reference, details) =
            normalize_line_metadata(&input(None, None, None)).unwrap();
        assert_eq!(source, DEFAULT_LINE_SOURCE);
        assert_eq!(reference, "");
        assert!(details.as_object().is_some_and(|object| object.is_empty()));
    }

    #[test]
    fn trims_plugin_metadata() {
        let (source, reference, details) = normalize_line_metadata(&input(
            Some("  mobile.device  "),
            Some("  device:7  "),
            Some(serde_json::json!({"imei": "123"})),
        ))
        .unwrap();
        assert_eq!(source, "mobile.device");
        assert_eq!(reference, "device:7");
        assert_eq!(details["imei"], "123");
    }

    #[test]
    fn rejects_oversized_source() {
        let long = "x".repeat(MAX_SOURCE_LEN + 1);
        assert!(normalize_line_metadata(&input(Some(&long), None, None)).is_err());
    }

    #[test]
    fn rejects_oversized_details() {
        let big = serde_json::json!({ "note": "x".repeat(MAX_DETAILS_BYTES) });
        assert!(normalize_line_metadata(&input(None, None, Some(big))).is_err());
    }

    #[test]
    fn maps_prepared_lines_to_the_event() {
        let sale = Sale {
            item_id: 1,
            variant_id: None,
            line_name: "Phone".into(),
            rate: 500,
            qty: 2,
            units: 1,
            amount: 1000,
            unit_cost: 0,
            cost_total: 0,
            source: "mobile.device".into(),
            reference: "device:7".into(),
            details: serde_json::json!({"imei": "123"}),
        };
        let lines = event_lines(&[sale]);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].quantity, "2");
        assert_eq!(lines[0].unit_amount, 500);
        assert_eq!(lines[0].amount, 1000);
        assert_eq!(lines[0].source, "mobile.device");
        assert_eq!(lines[0].details["imei"], "123");
    }
}
