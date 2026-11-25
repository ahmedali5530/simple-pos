use sqlx::PgPool;

use crate::billing::load_pos;
use crate::error::AppError;
use crate::models::{
    BarcodeRule, CategoryForm, LoginForm, LoginSession, PosSnapshot, PosUser, SavedRecord,
    SectionForm, SettingsForm, ShopConfig, UserForm,
};

fn friendly_unique(err: sqlx::Error) -> AppError {
    let constraint = err
        .as_database_error()
        .and_then(|db| db.constraint().map(str::to_string));
    match constraint.as_deref() {
        Some("sections_short_name_lower_idx") => {
            AppError::BadRequest("That short name is already used.".into())
        }
        Some("categories_name_key") | Some("categories_name_lower_idx") => {
            AppError::BadRequest("That category name is already used.".into())
        }
        Some("users_name_lower_idx") => AppError::BadRequest("That name is already used.".into()),
        Some("barcode_rules_prefix_key") => {
            AppError::BadRequest("Each barcode prefix is used once.".into())
        }
        _ => AppError::Db(err),
    }
}

fn hex_color(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 7
        && bytes[0] == b'#'
        && bytes[1..].iter().all(|byte| byte.is_ascii_hexdigit())
}

async fn taken(
    pool: &PgPool,
    sql: &str,
    name: &str,
    id: Option<i32>,
) -> Result<bool, AppError> {
    let found = sqlx::query_scalar::<_, i32>(sql)
        .bind(name)
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(found.is_some())
}

pub async fn save_section(
    pool: &PgPool,
    shop: &ShopConfig,
    id: Option<i32>,
    form: &SectionForm,
) -> Result<SavedRecord, AppError> {
    let name = form.name.trim();
    let short_name = form.short_name.trim();
    if name.is_empty() {
        return Err(AppError::BadRequest("Enter a section name.".into()));
    }
    if name.chars().count() > 80 {
        return Err(AppError::BadRequest(
            "Keep the section name to 80 characters.".into(),
        ));
    }
    if short_name.is_empty() {
        return Err(AppError::BadRequest(
            "Enter the short name shown on the tab.".into(),
        ));
    }
    if short_name.chars().count() > 24 {
        return Err(AppError::BadRequest(
            "Keep the short name to 24 characters.".into(),
        ));
    }
    if form.kind != "retail" && form.kind != "food" {
        return Err(AppError::BadRequest("Choose retail or food.".into()));
    }
    let accent = form.accent.trim().to_ascii_lowercase();
    if !hex_color(&accent) {
        return Err(AppError::BadRequest("Choose a color for the section.".into()));
    }
    if taken(
        pool,
        "SELECT id FROM sections WHERE lower(short_name) = lower($1) AND id IS DISTINCT FROM $2",
        short_name,
        id,
    )
    .await?
    {
        return Err(AppError::BadRequest(
            "That short name is already used.".into(),
        ));
    }

    let saved_id = if let Some(id) = id {
        let updated = sqlx::query(
            "UPDATE sections SET name = $1, short_name = $2, kind = $3, accent = $4 WHERE id = $5",
        )
        .bind(name)
        .bind(short_name)
        .bind(&form.kind)
        .bind(&accent)
        .bind(id)
        .execute(pool)
        .await
        .map_err(friendly_unique)?;
        if updated.rows_affected() == 0 {
            return Err(AppError::NotFound(
                "That section is no longer in the catalog.".into(),
            ));
        }
        id
    } else {
        let order = sqlx::query_scalar::<_, i32>(
            "SELECT COALESCE(MAX(pos_order), -1) + 1 FROM sections",
        )
        .fetch_one(pool)
        .await?;
        sqlx::query_scalar::<_, i32>(
            r#"
            INSERT INTO sections (name, short_name, kind, accent, show_on_pos, pos_order)
            VALUES ($1, $2, $3, $4, TRUE, $5)
            RETURNING id
            "#,
        )
        .bind(name)
        .bind(short_name)
        .bind(&form.kind)
        .bind(&accent)
        .bind(order)
        .fetch_one(pool)
        .await
        .map_err(friendly_unique)?
    };

    Ok(SavedRecord {
        id: saved_id,
        pos: load_pos(pool, shop).await?,
    })
}

pub fn ensure_manager(role: &str) -> Result<(), AppError> {
    if role != "manager" {
        return Err(AppError::Forbidden(
            "Only a manager can remove this.".into(),
        ));
    }
    Ok(())
}

pub async fn remove_section(
    pool: &PgPool,
    shop: &ShopConfig,
    id: i32,
) -> Result<PosSnapshot, AppError> {
    let total = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM sections")
        .fetch_one(pool)
        .await?;
    if total <= 1 {
        return Err(AppError::BadRequest(
            "Keep at least one section on the till.".into(),
        ));
    }
    let exists = sqlx::query_scalar::<_, i32>("SELECT id FROM sections WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    if exists.is_none() {
        return Err(AppError::NotFound(
            "That section is no longer in the catalog.".into(),
        ));
    }
    let items = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM items WHERE section_id = $1")
        .bind(id)
        .fetch_one(pool)
        .await?;
    if items > 0 {
        return Err(AppError::BadRequest(
            "Move this section's items before removing it.".into(),
        ));
    }
    let bills = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM bills WHERE section_id = $1")
        .bind(id)
        .fetch_one(pool)
        .await?;
    if bills > 0 {
        return Err(AppError::BadRequest(
            "This section has bills, so it stays on record.".into(),
        ));
    }
    sqlx::query("DELETE FROM sections WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    load_pos(pool, shop).await
}

pub async fn reorder_sections(
    pool: &PgPool,
    shop: &ShopConfig,
    ids: &[i32],
) -> Result<PosSnapshot, AppError> {
    apply_order(pool, shop, ids, "sections", "pos_order", "List every section when changing the order.").await
}

pub async fn save_category(
    pool: &PgPool,
    shop: &ShopConfig,
    id: Option<i32>,
    form: &CategoryForm,
) -> Result<SavedRecord, AppError> {
    let name = form.name.trim();
    if name.is_empty() {
        return Err(AppError::BadRequest("Enter a category name.".into()));
    }
    if name.chars().count() > 40 {
        return Err(AppError::BadRequest(
            "Keep the category name to 40 characters.".into(),
        ));
    }
    if taken(
        pool,
        "SELECT id FROM categories WHERE lower(name) = lower($1) AND id IS DISTINCT FROM $2",
        name,
        id,
    )
    .await?
    {
        return Err(AppError::BadRequest(
            "That category name is already used.".into(),
        ));
    }

    let saved_id = if let Some(id) = id {
        let updated = sqlx::query("UPDATE categories SET name = $1 WHERE id = $2")
            .bind(name)
            .bind(id)
            .execute(pool)
            .await
            .map_err(friendly_unique)?;
        if updated.rows_affected() == 0 {
            return Err(AppError::NotFound(
                "That category is no longer in the catalog.".into(),
            ));
        }
        id
    } else {
        let order = sqlx::query_scalar::<_, i32>(
            "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM categories",
        )
        .fetch_one(pool)
        .await?;
        sqlx::query_scalar::<_, i32>(
            "INSERT INTO categories (name, sort_order) VALUES ($1, $2) RETURNING id",
        )
        .bind(name)
        .bind(order)
        .fetch_one(pool)
        .await
        .map_err(friendly_unique)?
    };

    Ok(SavedRecord {
        id: saved_id,
        pos: load_pos(pool, shop).await?,
    })
}

pub async fn remove_category(
    pool: &PgPool,
    shop: &ShopConfig,
    id: i32,
) -> Result<PosSnapshot, AppError> {
    let total = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM categories")
        .fetch_one(pool)
        .await?;
    if total <= 1 {
        return Err(AppError::BadRequest(
            "Keep at least one category.".into(),
        ));
    }
    let exists = sqlx::query_scalar::<_, i32>("SELECT id FROM categories WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    if exists.is_none() {
        return Err(AppError::NotFound(
            "That category is no longer in the catalog.".into(),
        ));
    }
    let items = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM items WHERE category_id = $1")
        .bind(id)
        .fetch_one(pool)
        .await?;
    if items > 0 {
        return Err(AppError::BadRequest(
            "Move this category's items before removing it.".into(),
        ));
    }
    sqlx::query("DELETE FROM categories WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    load_pos(pool, shop).await
}

pub async fn remove_item(
    pool: &PgPool,
    shop: &ShopConfig,
    id: i32,
) -> Result<PosSnapshot, AppError> {
    let active = sqlx::query_scalar::<_, bool>("SELECT active FROM items WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    let Some(active) = active else {
        return Err(AppError::NotFound(
            "That item is no longer in the catalog.".into(),
        ));
    };
    if !active {
        return Err(AppError::BadRequest("That item is already removed.".into()));
    }
    let bill_lines = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM bill_lines WHERE item_id = $1")
        .bind(id)
        .fetch_one(pool)
        .await?;
    let movements =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM stock_movements WHERE item_id = $1")
            .bind(id)
            .fetch_one(pool)
            .await?;
    if bill_lines > 0 || movements > 0 {
        sqlx::query("UPDATE items SET active = FALSE WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
    } else {
        sqlx::query("DELETE FROM items WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
    }
    load_pos(pool, shop).await
}

pub async fn reorder_categories(
    pool: &PgPool,
    shop: &ShopConfig,
    ids: &[i32],
) -> Result<PosSnapshot, AppError> {
    apply_order(
        pool,
        shop,
        ids,
        "categories",
        "sort_order",
        "List every category when changing the order.",
    )
    .await
}

pub async fn save_user(
    pool: &PgPool,
    shop: &ShopConfig,
    id: Option<i32>,
    form: &UserForm,
) -> Result<SavedRecord, AppError> {
    let name = form.name.trim();
    if name.is_empty() {
        return Err(AppError::BadRequest("Enter a name.".into()));
    }
    if name.chars().count() > 40 {
        return Err(AppError::BadRequest("Keep the name to 40 characters.".into()));
    }
    if form.role != "cashier" && form.role != "manager" {
        return Err(AppError::BadRequest("Choose cashier or manager.".into()));
    }
    let password = form
        .password
        .as_ref()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty());
    if id.is_none() && password.is_none() {
        return Err(AppError::BadRequest("Enter a password.".into()));
    }
    if let Some(password) = password {
        if password.chars().count() < 4 {
            return Err(AppError::BadRequest(
                "Use at least 4 characters for the password.".into(),
            ));
        }
    }
    if taken(
        pool,
        "SELECT id FROM users WHERE lower(name) = lower($1) AND id IS DISTINCT FROM $2",
        name,
        id,
    )
    .await?
    {
        return Err(AppError::BadRequest("That name is already used.".into()));
    }
    if !keeps_a_manager(pool, id, form.role == "manager" && form.active).await? {
        return Err(AppError::BadRequest("Keep at least one manager.".into()));
    }

    let saved_id = if let Some(id) = id {
        let updated = if let Some(password) = password {
            let hash = hash_password(password)?;
            sqlx::query(
                "UPDATE users SET name = $1, password_hash = $2, role = $3, active = $4 WHERE id = $5",
            )
            .bind(name)
            .bind(hash)
            .bind(&form.role)
            .bind(form.active)
            .bind(id)
            .execute(pool)
            .await
            .map_err(friendly_unique)?
        } else {
            sqlx::query("UPDATE users SET name = $1, role = $2, active = $3 WHERE id = $4")
                .bind(name)
                .bind(&form.role)
                .bind(form.active)
                .bind(id)
                .execute(pool)
                .await
                .map_err(friendly_unique)?
        };
        if updated.rows_affected() == 0 {
            return Err(AppError::NotFound(
                "That user is no longer saved.".into(),
            ));
        }
        id
    } else {
        let hash = hash_password(password.unwrap_or_default())?;
        sqlx::query_scalar::<_, i32>(
            r#"
            INSERT INTO users (name, password_hash, role, active)
            VALUES ($1, $2, $3, $4)
            RETURNING id
            "#,
        )
        .bind(name)
        .bind(hash)
        .bind(&form.role)
        .bind(form.active)
        .fetch_one(pool)
        .await
        .map_err(friendly_unique)?
    };

    Ok(SavedRecord {
        id: saved_id,
        pos: load_pos(pool, shop).await?,
    })
}

pub async fn remove_user(
    pool: &PgPool,
    shop: &ShopConfig,
    id: i32,
) -> Result<PosSnapshot, AppError> {
    let user = sqlx::query_as::<_, PosUser>("SELECT id, name, role, active FROM users WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    if user.is_none() {
        return Err(AppError::NotFound("That user is no longer saved.".into()));
    }
    if !keeps_a_manager(pool, Some(id), false).await? {
        return Err(AppError::BadRequest("Keep at least one manager.".into()));
    }
    let sales = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM bills WHERE user_id = $1")
        .bind(id)
        .fetch_one(pool)
        .await?;
    if sales > 0 {
        return Err(AppError::BadRequest(
            "This user has sales, so the account stays on record.".into(),
        ));
    }
    sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    load_pos(pool, shop).await
}

pub async fn needs_first_user(pool: &PgPool) -> Result<bool, AppError> {
    let count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM users")
        .fetch_one(pool)
        .await?;
    Ok(count == 0)
}

pub async fn login(pool: &PgPool, form: &LoginForm) -> Result<LoginSession, AppError> {
    let count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM users")
        .fetch_one(pool)
        .await?;
    if count == 0 {
        return Err(AppError::BadRequest(
            "Sign-in starts once users are saved in Settings.".into(),
        ));
    }
    let name = form.name.trim();
    let row = sqlx::query_as::<_, (i32, String, String, String, bool)>(
        "SELECT id, name, password_hash, role, active FROM users WHERE lower(name) = lower($1)",
    )
    .bind(name)
    .fetch_optional(pool)
    .await?;
    let Some((id, stored_name, hash, role, active)) = row else {
        return Err(AppError::BadRequest(
            "That name or password does not match.".into(),
        ));
    };
    let matches = bcrypt::verify(form.password.as_str(), &hash).unwrap_or(false);
    if !matches {
        return Err(AppError::BadRequest(
            "That name or password does not match.".into(),
        ));
    }
    if !active {
        return Err(AppError::BadRequest("That account is turned off.".into()));
    }
    let token = new_token()?;
    sqlx::query("INSERT INTO sessions (token, user_id) VALUES ($1, $2)")
        .bind(&token)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(LoginSession {
        id,
        name: stored_name,
        role,
        active,
        token,
    })
}

pub struct Cashier {
    pub id: i32,
    pub name: String,
    pub role: String,
}

pub async fn open_session(pool: &PgPool, token: &str) -> Result<Cashier, AppError> {
    let token = token.trim();
    if token.is_empty() {
        return Err(AppError::Unauthorized(
            "Sign in before posting a bill.".into(),
        ));
    }
    let row = sqlx::query_as::<_, (i32, String, String, bool)>(
        r#"
        SELECT u.id, u.name, u.role, u.active
        FROM sessions s
        JOIN users u ON u.id = s.user_id
        WHERE s.token = $1
        "#,
    )
    .bind(token)
    .fetch_optional(pool)
    .await?;
    let Some((id, name, role, active)) = row else {
        return Err(AppError::Unauthorized(
            "Sign in before posting a bill.".into(),
        ));
    };
    if !active {
        return Err(AppError::Unauthorized("That account is turned off.".into()));
    }
    Ok(Cashier { id, name, role })
}

pub async fn logout(pool: &PgPool, token: &str) -> Result<(), AppError> {
    sqlx::query("DELETE FROM sessions WHERE token = $1")
        .bind(token.trim())
        .execute(pool)
        .await?;
    Ok(())
}

fn new_token() -> Result<String, AppError> {
    let mut bytes = [0_u8; 32];
    getrandom::getrandom(&mut bytes).map_err(|_| {
        AppError::BadRequest("Could not start the session.".into())
    })?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

pub async fn save_settings(
    pool: &PgPool,
    fallback: &ShopConfig,
    form: &SettingsForm,
) -> Result<PosSnapshot, AppError> {
    let shop = ShopConfig {
        name: form.name.clone(),
        currency: form.currency.clone(),
        locale: form.locale.clone(),
        bill_prefix: form.bill_prefix.clone(),
        barcode_rules: Vec::new(),
    };
    let shop = shop.checked().map_err(|message| {
        let text = if message.contains("currency") {
            "Currency is a 3-letter code, such as PKR."
        } else if message.contains("locale") {
            "Enter a locale, such as en-PK."
        } else if message.contains("bill_prefix") {
            "The bill prefix is 1 to 8 letters or digits."
        } else {
            "Enter a shop name."
        };
        AppError::BadRequest(text.into())
    })?;
    if form.barcode_rules.len() > 20 {
        return Err(AppError::BadRequest("Keep barcode rules to 20.".into()));
    }
    let mut seen = std::collections::HashSet::new();
    for rule in &form.barcode_rules {
        check_rule(rule)?;
        if !seen.insert(rule.prefix.clone()) {
            return Err(AppError::BadRequest(
                "Each barcode prefix is used once.".into(),
            ));
        }
    }

    let printer_kind = form.printer_kind.trim().to_string();
    if printer_kind != "network" && printer_kind != "usb" && printer_kind != "serial" {
        return Err(AppError::BadRequest(
            "The printer is network, usb, or serial.".into(),
        ));
    }
    if form.printer_port < 1 || form.printer_port > 1_000_000 {
        return Err(AppError::BadRequest(
            "The printer port is not a usable number.".into(),
        ));
    }
    let printer_target = form.printer_target.trim().to_string();
    if form.print_enabled && printer_kind != "usb" && printer_target.is_empty() {
        return Err(AppError::BadRequest(
            "Enter the printer address.".into(),
        ));
    }
    let printer_command_set = form.printer_command_set.trim().to_lowercase();
    if !matches!(printer_command_set.as_str(), "escpos" | "star" | "star-graphic") {
        return Err(AppError::BadRequest(
            "The printer commands are ESC/POS, Star, or Star graphic.".into(),
        ));
    }
    if !matches!(form.printer_paper_width, 58 | 72 | 80) {
        return Err(AppError::BadRequest(
            "The printer paper is 58, 72, or 80 mm.".into(),
        ));
    }

    let mut tx = pool.begin().await?;
    sqlx::query(
        r#"
        INSERT INTO shop_settings (
            id, name, currency, locale, bill_prefix, print_enabled, printer_kind, printer_target, printer_port, printer_command_set, printer_paper_width
        )
        VALUES (1, $1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
        ON CONFLICT (id) DO UPDATE
        SET name = EXCLUDED.name,
            currency = EXCLUDED.currency,
            locale = EXCLUDED.locale,
            bill_prefix = EXCLUDED.bill_prefix,
            print_enabled = EXCLUDED.print_enabled,
            printer_kind = EXCLUDED.printer_kind,
            printer_target = EXCLUDED.printer_target,
            printer_port = EXCLUDED.printer_port,
            printer_command_set = EXCLUDED.printer_command_set,
            printer_paper_width = EXCLUDED.printer_paper_width
        "#,
    )
    .bind(&shop.name)
    .bind(&shop.currency)
    .bind(&shop.locale)
    .bind(&shop.bill_prefix)
    .bind(form.print_enabled)
    .bind(&printer_kind)
    .bind(&printer_target)
    .bind(form.printer_port)
    .bind(&printer_command_set)
    .bind(form.printer_paper_width)
    .execute(&mut *tx)
    .await?;
    sqlx::query("DELETE FROM barcode_rules")
        .execute(&mut *tx)
        .await?;
    for (index, rule) in form.barcode_rules.iter().enumerate() {
        sqlx::query(
            "INSERT INTO barcode_rules (prefix, mode, scale, sort_order) VALUES ($1, $2, $3, $4)",
        )
        .bind(&rule.prefix)
        .bind(&rule.mode)
        .bind(rule.scale)
        .bind(index as i32)
        .execute(&mut *tx)
        .await
        .map_err(friendly_unique)?;
    }
    tx.commit().await?;
    load_pos(pool, fallback).await
}

fn check_rule(rule: &BarcodeRule) -> Result<(), AppError> {
    match rule.clone().checked() {
        Ok(_) => Ok(()),
        Err(message) if message.contains("prefix must") => Err(AppError::BadRequest(
            "A barcode prefix is 1 to 4 digits.".into(),
        )),
        Err(message) if message.contains("mode") => Err(AppError::BadRequest(
            "Each barcode rule is weight or price.".into(),
        )),
        Err(message) if message.contains("scale") => Err(AppError::BadRequest(
            "Barcode scale must be at least 1.".into(),
        )),
        Err(message) if message.contains("room") => Err(AppError::BadRequest(
            "That prefix leaves no room for an item code.".into(),
        )),
        Err(message) => Err(AppError::BadRequest(message)),
    }
}

fn hash_password(password: &str) -> Result<String, AppError> {
    bcrypt::hash(password, 10).map_err(|_| {
        AppError::BadRequest("Could not save that password.".into())
    })
}

async fn keeps_a_manager(
    pool: &PgPool,
    changing_id: Option<i32>,
    next_is_manager: bool,
) -> Result<bool, AppError> {
    let others = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COUNT(*)
        FROM users
        WHERE role = 'manager' AND active AND id IS DISTINCT FROM $1
        "#,
    )
    .bind(changing_id)
    .fetch_one(pool)
    .await?;
    Ok(others > 0 || next_is_manager)
}

async fn apply_order(
    pool: &PgPool,
    shop: &ShopConfig,
    ids: &[i32],
    table: &str,
    column: &str,
    incomplete: &str,
) -> Result<PosSnapshot, AppError> {
    if ids.is_empty() {
        return Err(AppError::BadRequest(incomplete.into()));
    }
    let mut seen = std::collections::HashSet::new();
    if ids.iter().any(|id| !seen.insert(*id)) {
        return Err(AppError::BadRequest("Each record can only appear once.".into()));
    }
    let total = sqlx::query_scalar::<_, i64>(&format!("SELECT COUNT(*) FROM {table}"))
        .fetch_one(pool)
        .await?;
    if total != ids.len() as i64 {
        return Err(AppError::BadRequest(incomplete.into()));
    }
    let known = sqlx::query_scalar::<_, i64>(&format!("SELECT COUNT(*) FROM {table} WHERE id = ANY($1)"))
        .bind(ids)
        .fetch_one(pool)
        .await?;
    if known != ids.len() as i64 {
        return Err(AppError::NotFound(
            "One of those records is no longer saved.".into(),
        ));
    }
    let mut tx = pool.begin().await?;
    for (index, id) in ids.iter().enumerate() {
        sqlx::query(&format!("UPDATE {table} SET {column} = $1 WHERE id = $2"))
            .bind(index as i32)
            .bind(id)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    load_pos(pool, shop).await
}
