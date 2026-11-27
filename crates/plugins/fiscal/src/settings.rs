use platform_kernel::context::TenantTx;
use platform_kernel::error::AppError;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct FiscalSettings {
    pub authority: String,
    pub mode: String,
    pub ntn: String,
    pub strn: String,
    pub pos_id: String,
    pub api_token: String,
    pub api_base_url: String,
    pub business_name: String,
    pub province: String,
    pub address: String,
    pub scenario_id: String,
    pub hs_code: String,
    pub uom: String,
    pub sale_type: String,
    pub st_rate: String,
    pub tax_inclusive: bool,
}

impl Default for FiscalSettings {
    fn default() -> Self {
        Self {
            authority: "fbr".into(),
            mode: "demo".into(),
            ntn: String::new(),
            strn: String::new(),
            pos_id: String::new(),
            api_token: String::new(),
            api_base_url: String::new(),
            business_name: String::new(),
            province: String::new(),
            address: String::new(),
            scenario_id: "SN001".into(),
            hs_code: "0101.2100".into(),
            uom: "Numbers, pieces, units".into(),
            sale_type: "Goods at standard rate (default)".into(),
            st_rate: "18%".into(),
            tax_inclusive: true,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct FiscalSettingsForm {
    pub authority: String,
    pub mode: String,
    pub ntn: String,
    pub strn: String,
    pub pos_id: String,
    pub api_token: String,
    pub api_base_url: String,
    pub business_name: String,
    pub province: String,
    pub address: String,
    pub scenario_id: String,
    pub hs_code: String,
    pub uom: String,
    pub sale_type: String,
    pub st_rate: String,
    pub tax_inclusive: bool,
}

impl FiscalSettingsForm {
    pub fn checked(self) -> Result<FiscalSettings, AppError> {
        let authority = self.authority.trim().to_ascii_lowercase();
        if authority != "fbr" && authority != "pra" {
            return Err(AppError::BadRequest(
                "Authority must be FBR or PRA.".into(),
            ));
        }
        let mode = self.mode.trim().to_ascii_lowercase();
        if mode != "demo" && mode != "sandbox" && mode != "live" {
            return Err(AppError::BadRequest(
                "Mode must be demo, sandbox, or live.".into(),
            ));
        }
        Ok(FiscalSettings {
            authority,
            mode,
            ntn: self.ntn.trim().to_string(),
            strn: self.strn.trim().to_string(),
            pos_id: self.pos_id.trim().to_string(),
            api_token: self.api_token.trim().to_string(),
            api_base_url: self.api_base_url.trim().trim_end_matches('/').to_string(),
            business_name: self.business_name.trim().to_string(),
            province: self.province.trim().to_string(),
            address: self.address.trim().to_string(),
            scenario_id: {
                let value = self.scenario_id.trim().to_string();
                if value.is_empty() {
                    "SN001".into()
                } else {
                    value
                }
            },
            hs_code: {
                let value = self.hs_code.trim().to_string();
                if value.is_empty() {
                    "0101.2100".into()
                } else {
                    value
                }
            },
            uom: {
                let value = self.uom.trim().to_string();
                if value.is_empty() {
                    "Numbers, pieces, units".into()
                } else {
                    value
                }
            },
            sale_type: {
                let value = self.sale_type.trim().to_string();
                if value.is_empty() {
                    "Goods at standard rate (default)".into()
                } else {
                    value
                }
            },
            st_rate: {
                let value = self.st_rate.trim().to_string();
                if value.is_empty() {
                    "18%".into()
                } else if value.ends_with('%') {
                    value
                } else {
                    format!("{value}%")
                }
            },
            tax_inclusive: self.tax_inclusive,
        })
    }
}

pub async fn load(tx: &mut TenantTx<'_>) -> Result<FiscalSettings, AppError> {
    let row = sqlx::query_as::<_, FiscalSettings>(
        r#"
        SELECT authority, mode, ntn, strn, pos_id, api_token, api_base_url,
               business_name, province, address, scenario_id, hs_code, uom,
               sale_type, st_rate, tax_inclusive
        FROM fiscal_settings
        WHERE tenant_id = current_setting('app.tenant_id')::uuid
        "#,
    )
    .fetch_optional(tx.conn())
    .await?;
    Ok(row.unwrap_or_default())
}

pub async fn save(tx: &mut TenantTx<'_>, settings: &FiscalSettings) -> Result<(), AppError> {
    sqlx::query(
        r#"
        INSERT INTO fiscal_settings (
            tenant_id, authority, mode, ntn, strn, pos_id, api_token, api_base_url,
            business_name, province, address, scenario_id, hs_code, uom, sale_type,
            st_rate, tax_inclusive, updated_at
        ) VALUES (
            current_setting('app.tenant_id')::uuid, $1, $2, $3, $4, $5, $6, $7,
            $8, $9, $10, $11, $12, $13, $14, $15, $16, now()
        )
        ON CONFLICT (tenant_id) DO UPDATE SET
            authority = EXCLUDED.authority,
            mode = EXCLUDED.mode,
            ntn = EXCLUDED.ntn,
            strn = EXCLUDED.strn,
            pos_id = EXCLUDED.pos_id,
            api_token = EXCLUDED.api_token,
            api_base_url = EXCLUDED.api_base_url,
            business_name = EXCLUDED.business_name,
            province = EXCLUDED.province,
            address = EXCLUDED.address,
            scenario_id = EXCLUDED.scenario_id,
            hs_code = EXCLUDED.hs_code,
            uom = EXCLUDED.uom,
            sale_type = EXCLUDED.sale_type,
            st_rate = EXCLUDED.st_rate,
            tax_inclusive = EXCLUDED.tax_inclusive,
            updated_at = now()
        "#,
    )
    .bind(&settings.authority)
    .bind(&settings.mode)
    .bind(&settings.ntn)
    .bind(&settings.strn)
    .bind(&settings.pos_id)
    .bind(&settings.api_token)
    .bind(&settings.api_base_url)
    .bind(&settings.business_name)
    .bind(&settings.province)
    .bind(&settings.address)
    .bind(&settings.scenario_id)
    .bind(&settings.hs_code)
    .bind(&settings.uom)
    .bind(&settings.sale_type)
    .bind(&settings.st_rate)
    .bind(settings.tax_inclusive)
    .execute(tx.conn())
    .await?;
    Ok(())
}
