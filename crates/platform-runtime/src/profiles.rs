//! A profile is data: a list of plugin ids. The engine does not interpret them.

use platform_kernel::error::AppError;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct ProfileFile {
    plugins: Vec<String>,
}

pub fn from_json(text: &str) -> Result<Vec<String>, AppError> {
    let file: ProfileFile = serde_json::from_str(text).map_err(|err| AppError::BadRequest(err.to_string()))?;
    Ok(file.plugins)
}

pub fn plugins_for(profile: &str) -> Result<Vec<String>, AppError> {
    let text = match profile {
        "retail" => include_str!("../../../profiles/retail.json"),
        "restaurant" => include_str!("../../../profiles/restaurant.json"),
        "coffee_shop" => include_str!("../../../profiles/coffee_shop.json"),
        "butcher" => include_str!("../../../profiles/butcher.json"),
        "shoe_market" => include_str!("../../../profiles/shoe_market.json"),
        "tailor" => include_str!("../../../profiles/tailor.json"),
        "installment_shop" => include_str!("../../../profiles/installment_shop.json"),
        _ => return Err(AppError::NotFound(format!("Unknown profile `{profile}`."))),
    };
    from_json(text)
}
