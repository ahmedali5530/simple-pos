use crate::models::BarcodeRule;

#[derive(Debug, Clone)]
pub struct EmbeddedCode {
    pub mode: String,
    pub item_code: String,
    pub value: i32,
    pub scale: i32,
}

pub fn parse_embedded(code: &str, rules: &[BarcodeRule]) -> Option<EmbeddedCode> {
    if code.len() != 13 || !code.bytes().all(|byte| byte.is_ascii_digit()) || !ean13_ok(code) {
        return None;
    }
    for rule in rules {
        if !code.starts_with(&rule.prefix) {
            continue;
        }
        let item_digits = 12 - rule.prefix.len() - 5;
        if item_digits < 1 {
            continue;
        }
        let item_start = rule.prefix.len();
        let item_code = code[item_start..item_start + item_digits].to_string();
        let value: i32 = code[item_start + item_digits..12].parse().ok()?;
        return Some(EmbeddedCode {
            mode: rule.mode.clone(),
            item_code,
            value,
            scale: rule.scale,
        });
    }
    None
}

pub fn codes_match(stored: &str, embedded: &str) -> bool {
    let left = stored.trim_start_matches('0');
    let right = embedded.trim_start_matches('0');
    stored == embedded || (!left.is_empty() && left == right)
}

pub fn embedded_sale(
    unit_rate: i32,
    code: &EmbeddedCode,
) -> Result<(i32, Option<i32>, String), String> {
    match code.mode.as_str() {
        "price" => {
            let rate = div_round(code.value as i64, code.scale)?;
            if rate < 1 {
                return Err("That barcode price is below 1.".into());
            }
            Ok((rate, None, String::new()))
        }
        "weight" => {
            if code.value < 1 {
                return Err("That barcode weight is empty.".into());
            }
            let amount = div_round(
                (unit_rate as i64)
                    .checked_mul(code.value as i64)
                    .ok_or_else(|| "That weight price is too large.".to_string())?,
                code.scale,
            )?;
            if amount < 1 {
                return Err("That weight price is below 1.".into());
            }
            Ok((
                amount,
                Some(code.value),
                format_scaled(code.value, code.scale),
            ))
        }
        _ => Err("Unknown barcode rule.".into()),
    }
}

pub fn ean13_ok(code: &str) -> bool {
    let digits: Vec<u32> = code.chars().filter_map(|ch| ch.to_digit(10)).collect();
    if digits.len() != 13 {
        return false;
    }
    let sum: u32 = digits[..12]
        .iter()
        .enumerate()
        .map(|(index, digit)| digit * if index % 2 == 0 { 1 } else { 3 })
        .sum();
    let check = (10 - (sum % 10)) % 10;
    check == digits[12]
}

#[derive(Debug, Clone)]
pub struct PromptQuote {
    pub rate: i32,
    pub units: i32,
    pub label: String,
}

pub fn prompt_sale(
    prompt: &str,
    base_rate: i32,
    scale: i32,
    raw: &str,
) -> Result<PromptQuote, String> {
    if base_rate < 1 {
        return Err("Set a base rate above 0.".into());
    }
    if scale < 1 {
        return Err("Stock per 1 must be at least 1.".into());
    }
    match prompt {
        "quantity" => quote_quantity(base_rate, scale, raw),
        "price" => quote_price(base_rate, scale, raw),
        _ => Err("This item does not ask for a value.".into()),
    }
}

fn quote_quantity(base_rate: i32, scale: i32, raw: &str) -> Result<PromptQuote, String> {
    let milli = parse_milli(raw)?;
    if milli % 1000 == 0 {
        return Ok(PromptQuote {
            rate: base_rate,
            units: scale,
            label: String::new(),
        });
    }
    let priced = (base_rate as i64)
        .checked_mul(milli)
        .ok_or_else(|| "That quantity price is too large.".to_string())?;
    let taken = (scale as i64)
        .checked_mul(milli)
        .ok_or_else(|| "That quantity takes too much stock.".to_string())?;
    let rate =
        div_round(priced, 1000).map_err(|_| "That quantity price is too large.".to_string())?;
    let units =
        div_round(taken, 1000).map_err(|_| "That quantity takes too much stock.".to_string())?;
    if rate < 1 || units < 1 {
        return Err("That quantity is too small to sell.".into());
    }
    Ok(PromptQuote {
        rate,
        units,
        label: format_milli(milli),
    })
}

fn quote_price(base_rate: i32, scale: i32, raw: &str) -> Result<PromptQuote, String> {
    let price = parse_price(raw)?;
    let taken = (price as i64)
        .checked_mul(scale as i64)
        .ok_or_else(|| "That price takes too much stock.".to_string())?;
    let units =
        div_round(taken, base_rate).map_err(|_| "That price takes too much stock.".to_string())?;
    if units < 1 {
        return Err("That price is below one unit of stock.".into());
    }
    Ok(PromptQuote {
        rate: price,
        units,
        label: String::new(),
    })
}

fn parse_milli(raw: &str) -> Result<i64, String> {
    let raw = raw.trim();
    let mut parts = raw.split('.');
    let whole = parts.next().unwrap_or("");
    let frac = parts.next();
    if parts.next().is_some()
        || whole.is_empty()
        || !whole.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err("Enter a quantity like 1 or 1.25.".into());
    }
    if let Some(frac) = frac {
        if frac.is_empty() || frac.len() > 3 || !frac.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err("Use at most 3 decimal places.".into());
        }
    }
    let whole: i64 = whole
        .parse()
        .map_err(|_| "That quantity is too large.".to_string())?;
    let frac_num: i64 = match frac {
        Some(frac) if !frac.is_empty() => format!("{frac:0<3}")
            .parse()
            .map_err(|_| "That quantity is too large.".to_string())?,
        _ => 0,
    };
    let milli = whole
        .checked_mul(1000)
        .and_then(|value| value.checked_add(frac_num))
        .ok_or_else(|| "That quantity is too large.".to_string())?;
    if milli < 1 {
        return Err("Enter a quantity above 0.".into());
    }
    Ok(milli)
}

fn parse_price(raw: &str) -> Result<i32, String> {
    let raw = raw.trim();
    if raw.is_empty() || !raw.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err("Enter a whole price.".into());
    }
    let price: i32 = raw
        .parse()
        .map_err(|_| "That price is too large.".to_string())?;
    if price < 1 {
        return Err("Enter a price above 0.".into());
    }
    Ok(price)
}

fn format_milli(milli: i64) -> String {
    let whole = milli / 1000;
    let fraction = milli % 1000;
    if fraction == 0 {
        return whole.to_string();
    }
    let digits = format!("{fraction:03}");
    format!("{whole}.{}", digits.trim_end_matches('0'))
}

fn div_round(value: i64, scale: i32) -> Result<i32, String> {
    if scale < 1 {
        return Err("Barcode scale must be at least 1.".into());
    }
    let scale = scale as i64;
    let rounded = (value + scale / 2) / scale;
    i32::try_from(rounded).map_err(|_| "That barcode amount is too large.".to_string())
}

fn format_scaled(value: i32, scale: i32) -> String {
    let whole = value / scale;
    let fraction = value % scale;
    if fraction == 0 {
        return whole.to_string();
    }
    let width = scale.to_string().len() - 1;
    let digits = format!("{fraction:0width$}");
    format!("{whole}.{}", digits.trim_end_matches('0'))
}
