//! FBR Digital Invoicing (DI v1.12) and PRA stubs.
//!
//! - `demo` — local QR only (no network)
//! - `sandbox` — POST https://gw.fbr.gov.pk/di_data/v1/di/postinvoicedata_sb
//! - `live` — POST https://gw.fbr.gov.pk/di_data/v1/di/postinvoicedata
//!
//! Routing between sandbox and production is also determined by the Bearer token
//! FBR issued; we still pick the documented URL for each mode.

use platform_kernel::error::AppError;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::settings::FiscalSettings;

const FBR_SANDBOX_URL: &str = "https://gw.fbr.gov.pk/di_data/v1/di/postinvoicedata_sb";
const FBR_LIVE_URL: &str = "https://gw.fbr.gov.pk/di_data/v1/di/postinvoicedata";

#[derive(Debug, Clone)]
pub struct SubmitRequest {
    pub bill_id: i32,
    pub bill_no: String,
    pub total: i32,
    pub posted_at: String,
    pub customer: Option<String>,
    pub lines: Vec<SubmitLine>,
    pub shop_name: String,
}

#[derive(Debug, Clone)]
pub struct SubmitLine {
    pub label: String,
    pub quantity: String,
    pub amount: i64,
}

#[derive(Debug, Clone)]
pub struct SubmitResult {
    pub invoice_no: String,
    pub qr_payload: String,
    pub raw: serde_json::Value,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FbrResponse {
    #[serde(default)]
    invoice_number: String,
    #[serde(default)]
    dated: String,
    #[serde(default)]
    validation_response: Option<FbrValidation>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FbrValidation {
    #[serde(default)]
    status_code: String,
    #[serde(default)]
    status: String,
    #[serde(default)]
    error_code: String,
    #[serde(default)]
    error: String,
    #[serde(default)]
    invoice_statuses: Option<Vec<FbrItemStatus>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FbrItemStatus {
    #[serde(default)]
    error_code: String,
    #[serde(default)]
    error: String,
    #[serde(default)]
    status: String,
}

pub async fn submit(settings: &FiscalSettings, request: &SubmitRequest) -> Result<SubmitResult, AppError> {
    match settings.authority.as_str() {
        "pra" => {
            Err(AppError::BadRequest(
                "PRA e-invoicing is not wired yet. Switch authority to FBR for Digital Invoicing.".into(),
            ))
        }
        "fbr" => fbr_submit(settings, request).await,
        other => Err(AppError::BadRequest(format!("Unknown fiscal authority `{other}`."))),
    }
}

async fn fbr_submit(
    settings: &FiscalSettings,
    request: &SubmitRequest,
) -> Result<SubmitResult, AppError> {
    match settings.mode.as_str() {
        "demo" => Ok(demo_result(settings, request)),
        "sandbox" | "live" => fbr_post(settings, request).await,
        other => Err(AppError::BadRequest(format!("Unknown fiscal mode `{other}`."))),
    }
}

fn demo_result(settings: &FiscalSettings, request: &SubmitRequest) -> SubmitResult {
    let invoice_no = format!(
        "FBR-DEMO-{}",
        request.bill_no.replace(|c: char| !c.is_ascii_alphanumeric(), "")
    );
    SubmitResult {
        invoice_no: invoice_no.clone(),
        // FBR asks for the IRN string in the QR, not a URL.
        qr_payload: invoice_no.clone(),
        raw: json!({
            "mode": "demo",
            "authority": settings.authority,
            "invoiceNumber": invoice_no,
            "bill_no": request.bill_no,
            "total": request.total,
            "note": "Demo mode does not call FBR. Use sandbox or live with an API token.",
        }),
    }
}

fn post_url(settings: &FiscalSettings) -> Result<String, AppError> {
    if !settings.api_base_url.is_empty() {
        return Ok(settings.api_base_url.clone());
    }
    match settings.mode.as_str() {
        "sandbox" => Ok(FBR_SANDBOX_URL.into()),
        "live" => Ok(FBR_LIVE_URL.into()),
        other => Err(AppError::BadRequest(format!(
            "No FBR URL for mode `{other}`."
        ))),
    }
}

fn parse_rate_percent(rate: &str) -> f64 {
    rate.trim()
        .trim_end_matches('%')
        .trim()
        .parse::<f64>()
        .unwrap_or(18.0)
}

fn money_major(amount: i64) -> f64 {
    amount as f64
}

fn split_tax(amount: i64, rate_pct: f64, tax_inclusive: bool) -> (f64, f64) {
    let gross = money_major(amount);
    if rate_pct <= 0.0 {
        return (gross, 0.0);
    }
    if tax_inclusive {
        let excl = (gross * 100.0) / (100.0 + rate_pct);
        let tax = gross - excl;
        (
            (excl * 100.0).round() / 100.0,
            (tax * 100.0).round() / 100.0,
        )
    } else {
        let tax = (gross * rate_pct) / 100.0;
        (
            (gross * 100.0).round() / 100.0,
            (tax * 100.0).round() / 100.0,
        )
    }
}

fn build_fbr_payload(settings: &FiscalSettings, request: &SubmitRequest) -> Result<Value, AppError> {
    if settings.ntn.trim().is_empty() {
        return Err(AppError::BadRequest(
            "Set the seller NTN/CNIC in Fiscal settings.".into(),
        ));
    }
    let business = if settings.business_name.trim().is_empty() {
        request.shop_name.trim()
    } else {
        settings.business_name.trim()
    };
    if business.is_empty() {
        return Err(AppError::BadRequest(
            "Set the seller business name in Fiscal settings (or shop profile).".into(),
        ));
    }
    if settings.province.trim().is_empty() || settings.address.trim().is_empty() {
        return Err(AppError::BadRequest(
            "Set seller province and address in Fiscal settings.".into(),
        ));
    }
    if settings.api_token.trim().is_empty() {
        return Err(AppError::BadRequest(
            "Set the FBR API Bearer token in Fiscal settings.".into(),
        ));
    }

    let rate_pct = parse_rate_percent(&settings.st_rate);
    let invoice_date = request
        .posted_at
        .get(..10)
        .filter(|slice| slice.chars().filter(|c| *c == '-').count() == 2)
        .map(str::to_string)
        .unwrap_or_else(|| chrono::Local::now().format("%Y-%m-%d").to_string());

    let buyer_name = request
        .customer
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .unwrap_or("Walking Customer");

    let mut items = Vec::new();
    if request.lines.is_empty() {
        let (excl, tax) = split_tax(i64::from(request.total), rate_pct, settings.tax_inclusive);
        items.push(json!({
            "hsCode": settings.hs_code,
            "productDescription": format!("Bill {}", request.bill_no),
            "rate": settings.st_rate,
            "uoM": settings.uom,
            "quantity": 1.0,
            "totalValues": 0.0,
            "valueSalesExcludingST": excl,
            "fixedNotifiedValueOrRetailPrice": 0.0,
            "salesTaxApplicable": tax,
            "salesTaxWithheldAtSource": 0.0,
            "extraTax": 0.0,
            "furtherTax": 0.0,
            "sroScheduleNo": "",
            "fedPayable": 0.0,
            "discount": 0.0,
            "saleType": settings.sale_type,
            "sroItemSerialNo": ""
        }));
    } else {
        for line in &request.lines {
            let qty: f64 = line.quantity.parse::<f64>().unwrap_or(1.0).max(0.0001);
            let (excl, tax) = split_tax(line.amount, rate_pct, settings.tax_inclusive);
            items.push(json!({
                "hsCode": settings.hs_code,
                "productDescription": line.label,
                "rate": settings.st_rate,
                "uoM": settings.uom,
                "quantity": qty,
                "totalValues": 0.0,
                "valueSalesExcludingST": excl,
                "fixedNotifiedValueOrRetailPrice": 0.0,
                "salesTaxApplicable": tax,
                "salesTaxWithheldAtSource": 0.0,
                "extraTax": 0.0,
                "furtherTax": 0.0,
                "sroScheduleNo": "",
                "fedPayable": 0.0,
                "discount": 0.0,
                "saleType": settings.sale_type,
                "sroItemSerialNo": ""
            }));
        }
    }

    let mut body = json!({
        "invoiceType": "Sale Invoice",
        "invoiceDate": invoice_date,
        "sellerNTNCNIC": settings.ntn,
        "sellerBusinessName": business,
        "sellerProvince": settings.province,
        "sellerAddress": settings.address,
        "buyerNTNCNIC": "",
        "buyerBusinessName": buyer_name,
        "buyerProvince": settings.province,
        "buyerAddress": settings.address,
        "buyerRegistrationType": "Unregistered",
        "invoiceRefNo": request.bill_no,
        "items": items,
    });

    if settings.mode == "sandbox" {
        let scenario = if settings.scenario_id.trim().is_empty() {
            "SN001"
        } else {
            settings.scenario_id.trim()
        };
        body
            .as_object_mut()
            .expect("object")
            .insert("scenarioId".into(), json!(scenario));
    }

    Ok(body)
}

async fn fbr_post(
    settings: &FiscalSettings,
    request: &SubmitRequest,
) -> Result<SubmitResult, AppError> {
    let url = post_url(settings)?;
    let body = build_fbr_payload(settings, request)?;

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(45))
        .build()
        .map_err(|error| AppError::BadRequest(format!("Could not build HTTP client. {error}")))?;

    let response = client
        .post(&url)
        .bearer_auth(settings.api_token.trim())
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|error| {
            AppError::BadRequest(format!("FBR did not answer. {error}"))
        })?;

    let status = response.status();
    let raw: Value = response.json().await.map_err(|error| {
        AppError::BadRequest(format!("FBR returned a non-JSON body. {error}"))
    })?;

    if !status.is_success() {
        let message = raw
            .pointer("/validationResponse/error")
            .or_else(|| raw.get("error"))
            .or_else(|| raw.get("message"))
            .and_then(|value| value.as_str())
            .unwrap_or("FBR rejected the invoice.");
        return Err(AppError::BadRequest(format!(
            "{message} (HTTP {})",
            status.as_u16()
        )));
    }

    let parsed: FbrResponse = serde_json::from_value(raw.clone()).unwrap_or(FbrResponse {
        invoice_number: String::new(),
        dated: String::new(),
        validation_response: None,
    });

    if let Some(validation) = parsed.validation_response.as_ref() {
        let code = validation.status_code.trim();
        let status_text = validation.status.trim().to_ascii_lowercase();
        let item_error = validation
            .invoice_statuses
            .as_ref()
            .into_iter()
            .flatten()
            .find(|item| {
                item.status.eq_ignore_ascii_case("Invalid")
                    || (!item.error.is_empty() && item.error_code != "00")
            })
            .map(|item| {
                if item.error.is_empty() {
                    format!("FBR item error {}", item.error_code)
                } else {
                    item.error.clone()
                }
            });

        if code == "01" || status_text == "invalid" || item_error.is_some() {
            let message = if !validation.error.is_empty() {
                validation.error.clone()
            } else if let Some(item_error) = item_error {
                item_error
            } else if !validation.error_code.is_empty() {
                format!("FBR error {}", validation.error_code)
            } else {
                "FBR marked the invoice invalid.".into()
            };
            return Err(AppError::BadRequest(message));
        }
    }

    // Prefer top-level invoiceNumber; fall back to first item invoiceNo.
    let mut invoice_no = parsed.invoice_number.trim().to_string();
    if invoice_no.is_empty() {
        invoice_no = raw
            .pointer("/validationResponse/invoiceStatuses/0/invoiceNo")
            .and_then(|value| value.as_str())
            .unwrap_or("")
            .to_string();
    }
    if invoice_no.is_empty() {
        return Err(AppError::BadRequest(
            "FBR did not return an invoiceNumber (IRN).".into(),
        ));
    }

    Ok(SubmitResult {
        // QR must encode the IRN string itself.
        qr_payload: invoice_no.clone(),
        invoice_no,
        raw,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::FiscalSettings;

    #[test]
    fn sandbox_payload_includes_scenario_and_seller() {
        let settings = FiscalSettings {
            authority: "fbr".into(),
            mode: "sandbox".into(),
            ntn: "0786909".into(),
            business_name: "Corner Shop".into(),
            province: "Punjab".into(),
            address: "Lahore".into(),
            api_token: "token".into(),
            scenario_id: "SN001".into(),
            hs_code: "0101.2100".into(),
            uom: "Numbers, pieces, units".into(),
            sale_type: "Goods at standard rate (default)".into(),
            st_rate: "18%".into(),
            tax_inclusive: true,
            ..FiscalSettings::default()
        };
        let request = SubmitRequest {
            bill_id: 1,
            bill_no: "B-1".into(),
            total: 1180,
            posted_at: "2025-04-21 10:00 AM".into(),
            customer: None,
            shop_name: "Corner Shop".into(),
            lines: vec![SubmitLine {
                label: "Milk".into(),
                quantity: "2".into(),
                amount: 1180,
            }],
        };
        let body = build_fbr_payload(&settings, &request).unwrap();
        assert_eq!(body["scenarioId"], "SN001");
        assert_eq!(body["sellerNTNCNIC"], "0786909");
        assert_eq!(body["items"][0]["productDescription"], "Milk");
        assert!(body["items"][0]["valueSalesExcludingST"].as_f64().unwrap() > 0.0);
    }

    #[test]
    fn demo_does_not_need_token() {
        let settings = FiscalSettings {
            mode: "demo".into(),
            ..FiscalSettings::default()
        };
        let request = SubmitRequest {
            bill_id: 1,
            bill_no: "B-9".into(),
            total: 100,
            posted_at: String::new(),
            customer: None,
            shop_name: "Shop".into(),
            lines: vec![],
        };
        let result = demo_result(&settings, &request);
        assert!(result.invoice_no.contains("B9"));
        assert_eq!(result.qr_payload, result.invoice_no);
    }
}
