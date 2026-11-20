use std::collections::{HashMap, HashSet};
use std::io::Cursor;

use calamine::{Data, Reader, Xlsx};
use rust_xlsxwriter::{Color, Format, Workbook};
use serde::Serialize;
use sqlx::{PgPool, Postgres, Transaction};

use crate::billing::load_pos;
use crate::error::AppError;
use crate::models::{PosSnapshot, ShopConfig};

const MAX_ROWS: usize = 5000;

#[derive(Debug, Serialize)]
pub struct ItemImport {
    pub created: i64,
    pub updated: i64,
    pub options: i64,
    pub counted: i64,
    pub pos: PosSnapshot,
}

#[derive(Debug, Serialize)]
pub struct MovementImport {
    pub applied: i64,
    pub pos: PosSnapshot,
}

#[derive(Debug, Serialize)]
pub struct CategoryImport {
    pub created: i64,
    pub updated: i64,
    pub removed: i64,
    pub reordered: bool,
    pub pos: PosSnapshot,
}

#[derive(sqlx::FromRow)]
struct LoadedItem {
    id: i32,
    barcode: Option<String>,
    name: String,
}

#[derive(sqlx::FromRow)]
struct LoadedVariant {
    id: i32,
    item_id: i32,
    barcode: Option<String>,
}

#[derive(sqlx::FromRow)]
struct SectionRef {
    id: i32,
    name: String,
    short_name: String,
}

#[derive(sqlx::FromRow)]
struct CategoryRef {
    id: i32,
    name: String,
}

struct ItemDraft {
    row: u32,
    id: Option<i32>,
    barcode: Option<String>,
    name: String,
    section_id: i32,
    category_id: i32,
    rate: i32,
    cost: Option<i64>,
    prompt: String,
    base_rate: Option<i32>,
    scale: i32,
    counted: Option<i32>,
    opening: Option<i32>,
}

struct OptionDraft {
    row: u32,
    item_id: Option<i32>,
    item_barcode: Option<String>,
    option_id: Option<i32>,
    name: String,
    barcode: Option<String>,
    rate: i32,
    units: i32,
    remove: bool,
}

struct MoveDraft {
    row: u32,
    item_id: i32,
    item_name: String,
    qty: i32,
    note: String,
}

struct CategoryDraft {
    row: u32,
    id: Option<i32>,
    name: String,
    remove: bool,
}

pub async fn export_items(pool: &PgPool) -> Result<Vec<u8>, AppError> {
    let items = sqlx::query_as::<_, ItemSheetRow>(
        r#"
        SELECT
            i.id, i.barcode, i.name, s.short_name AS section, c.name AS category,
            i.rate, i.avg_cost, i.prompt, i.base_rate, i.scale, i.stock
        FROM items i
        JOIN sections s ON s.id = i.section_id
        JOIN categories c ON c.id = i.category_id
        WHERE i.active
        ORDER BY s.pos_order, i.sort_order, i.id
        "#,
    )
    .fetch_all(pool)
    .await?;
    let options = sqlx::query_as::<_, OptionSheetRow>(
        r#"
        SELECT
            i.id AS item_id, i.barcode AS item_barcode, i.name AS item_name,
            v.id AS option_id, v.name, v.barcode, v.rate, v.units
        FROM item_variants v
        JOIN items i ON i.id = v.item_id
        JOIN sections s ON s.id = i.section_id
        WHERE i.active
        ORDER BY s.pos_order, i.sort_order, i.id, v.sort_order, v.id
        "#,
    )
    .fetch_all(pool)
    .await?;

    let mut workbook = Workbook::new();
    let header = Format::new().set_bold().set_background_color(Color::RGB(0xE2E8F0));
    let text = Format::new().set_num_format("@");
    let wrap = Format::new().set_text_wrap();

    let items_sheet = workbook.add_worksheet();
    items_sheet.set_name("Items").sheet()?;
    let item_headers = [
        "id", "barcode", "name", "section", "category", "rate", "cost", "prompt", "base_rate", "scale", "stock", "counted",
    ];
    for (col, name) in item_headers.iter().enumerate() {
        items_sheet.write_string_with_format(0, col as u16, *name, &header).sheet()?;
    }
    for (index, item) in items.iter().enumerate() {
        let row = (index + 1) as u32;
        items_sheet.write_number(row, 0, item.id as f64).sheet()?;
        write_text(items_sheet, row, 1, item.barcode.as_deref().unwrap_or(""), &text)?;
        items_sheet.write_string(row, 2, &item.name).sheet()?;
        items_sheet.write_string(row, 3, &item.section).sheet()?;
        items_sheet.write_string(row, 4, &item.category).sheet()?;
        items_sheet.write_number(row, 5, item.rate as f64).sheet()?;
        write_text(items_sheet, row, 6, &cost_text(item.avg_cost), &text)?;
        items_sheet.write_string(row, 7, &item.prompt).sheet()?;
        if let Some(base) = item.base_rate {
            items_sheet.write_number(row, 8, base as f64).sheet()?;
        }
        items_sheet.write_number(row, 9, item.scale as f64).sheet()?;
        items_sheet.write_number(row, 10, item.stock as f64).sheet()?;
    }
    items_sheet.set_freeze_panes(1, 0).sheet()?;
    set_widths(items_sheet, &[10.0, 16.0, 28.0, 18.0, 16.0, 10.0, 12.0, 12.0, 12.0, 10.0, 10.0, 12.0])?;

    let options_sheet = workbook.add_worksheet();
    options_sheet.set_name("Options").sheet()?;
    let option_headers = [
        "item_id", "item_barcode", "item_name", "option_id", "name", "barcode", "rate", "units", "remove",
    ];
    for (col, name) in option_headers.iter().enumerate() {
        options_sheet.write_string_with_format(0, col as u16, *name, &header).sheet()?;
    }
    for (index, option) in options.iter().enumerate() {
        let row = (index + 1) as u32;
        options_sheet.write_number(row, 0, option.item_id as f64).sheet()?;
        write_text(options_sheet, row, 1, option.item_barcode.as_deref().unwrap_or(""), &text)?;
        options_sheet.write_string(row, 2, &option.item_name).sheet()?;
        options_sheet.write_number(row, 3, option.option_id as f64).sheet()?;
        options_sheet.write_string(row, 4, &option.name).sheet()?;
        write_text(options_sheet, row, 5, option.barcode.as_deref().unwrap_or(""), &text)?;
        options_sheet.write_number(row, 6, option.rate as f64).sheet()?;
        options_sheet.write_number(row, 7, option.units as f64).sheet()?;
    }
    options_sheet.set_freeze_panes(1, 0).sheet()?;
    set_widths(options_sheet, &[10.0, 16.0, 28.0, 12.0, 16.0, 16.0, 10.0, 10.0, 12.0])?;

    let guide = workbook.add_worksheet();
    guide.set_name("Guide").sheet()?;
    let lines = [
        "Items: one row per item. Leave id blank to add an item. An id, or a barcode that already exists, updates that item.",
        "section is the business head. category is the category name. prompt is none, quantity, or price.",
        "cost is the average cost of one stock unit. Leave it blank to keep the current cost.",
        "base_rate and scale are used when prompt is quantity or price.",
        "stock is the on-hand amount at export. Leave counted blank to keep it.",
        "Fill counted only for items you have recounted. The difference is saved as a stock movement.",
        "A new item can use stock or counted as its opening amount.",
        "Options: one row per Piece, Pack, Full, Half, or any other option.",
        "Point the option at an item with item_id or item_barcode. Leave option_id blank to add one.",
        "Put yes in remove to delete that option. Options you do not list are left as they are.",
        "Pictures are not part of this workbook.",
    ];
    for (index, line) in lines.iter().enumerate() {
        guide.write_string_with_format(index as u32, 0, *line, &wrap).sheet()?;
    }
    guide.set_column_width(0, 110).sheet()?;
    workbook.save_to_buffer().sheet()
}

pub async fn export_movements(pool: &PgPool) -> Result<Vec<u8>, AppError> {
    let rows = sqlx::query_as::<_, MovementSheetRow>(
        r#"
        SELECT
            m.id,
            to_char(m.created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD HH24:MI') AS time_utc,
            i.barcode,
            i.name AS item,
            s.short_name AS section,
            m.qty,
            m.reason,
            b.bill_no
        FROM stock_movements m
        JOIN items i ON i.id = m.item_id
        JOIN sections s ON s.id = i.section_id
        LEFT JOIN bills b ON b.id = m.bill_id
        ORDER BY m.created_at DESC, m.id DESC
        LIMIT 5000
        "#,
    )
    .fetch_all(pool)
    .await?;

    let mut workbook = Workbook::new();
    let header = Format::new().set_bold().set_background_color(Color::RGB(0xE2E8F0));
    let text = Format::new().set_num_format("@");
    let wrap = Format::new().set_text_wrap();
    let sheet = workbook.add_worksheet();
    sheet.set_name("Movements").sheet()?;
    let headers = ["id", "time_utc", "barcode", "item", "section", "qty", "direction", "note", "bill"];
    for (col, name) in headers.iter().enumerate() {
        sheet.write_string_with_format(0, col as u16, *name, &header).sheet()?;
    }
    for (index, row) in rows.iter().enumerate() {
        let line = (index + 1) as u32;
        sheet.write_number(line, 0, row.id as f64).sheet()?;
        sheet.write_string(line, 1, &row.time_utc).sheet()?;
        write_text(sheet, line, 2, row.barcode.as_deref().unwrap_or(""), &text)?;
        sheet.write_string(line, 3, &row.item).sheet()?;
        sheet.write_string(line, 4, &row.section).sheet()?;
        sheet.write_number(line, 5, row.qty.abs() as f64).sheet()?;
        sheet.write_string(line, 6, if row.qty < 0 { "out" } else { "in" }).sheet()?;
        sheet.write_string(line, 7, &row.reason).sheet()?;
        write_text(sheet, line, 8, row.bill_no.as_deref().unwrap_or(""), &text)?;
    }
    sheet.set_freeze_panes(1, 0).sheet()?;
    set_widths(sheet, &[10.0, 18.0, 16.0, 28.0, 18.0, 10.0, 12.0, 36.0, 14.0])?;

    let guide = workbook.add_worksheet();
    guide.set_name("Guide").sheet()?;
    let lines = [
        "Rows with an id are history. Import skips them, so uploading this file again does not repeat old movements.",
        "Add a row with a blank id, the item barcode, a qty, and a note.",
        "Put in or out in direction and a positive qty. A blank direction uses a positive qty as stock in and a negative qty as stock out.",
        "You can use the item name instead of a barcode when that name belongs to one item.",
        "A variant barcode counts against its item. The whole file is saved together.",
        "This sheet shows the latest 5000 movements.",
    ];
    for (index, line) in lines.iter().enumerate() {
        guide.write_string_with_format(index as u32, 0, *line, &wrap).sheet()?;
    }
    guide.set_column_width(0, 110).sheet()?;
    workbook.save_to_buffer().sheet()
}

pub async fn import_items(pool: &PgPool, shop: &ShopConfig, bytes: &[u8]) -> Result<ItemImport, AppError> {
    let mut book = open_book(bytes)?;
    let Some(items_range) = named_range(&mut book, "Items")? else {
        return Err(AppError::BadRequest(
            "The workbook needs an Items sheet. Export a fresh file to get the columns.".into(),
        ));
    };
    let option_range = named_range(&mut book, "Options")?;
    let mut errors = Vec::new();
    let item_rows = read_rows(&items_range);
    let option_rows = option_range.as_ref().map(|range| read_rows(range)).unwrap_or_default();
    if item_rows.len() > MAX_ROWS || option_rows.len() > MAX_ROWS {
        return Err(AppError::BadRequest("A sheet can hold up to 5000 rows.".into()));
    }

    let sections = sqlx::query_as::<_, SectionRef>(
        "SELECT id, name, short_name FROM sections ORDER BY pos_order, id",
    )
    .fetch_all(pool)
    .await?;
    let categories = sqlx::query_as::<_, CategoryRef>("SELECT id, name FROM categories ORDER BY sort_order, id")
        .fetch_all(pool)
        .await?;
    let existing = sqlx::query_as::<_, LoadedItem>(
        "SELECT id, barcode, name FROM items WHERE active",
    )
    .fetch_all(pool)
    .await?;
    let variants = sqlx::query_as::<_, LoadedVariant>(
        r#"
        SELECT v.id, v.item_id, v.barcode
        FROM item_variants v
        JOIN items i ON i.id = v.item_id
        WHERE i.active
        "#,
    )
    .fetch_all(pool)
    .await?;

    let item_header = header_map(item_rows.first().map(|(_, row)| row.as_slice()).unwrap_or(&[]));
    let drafts = parse_items(&item_header, &item_rows, &sections, &categories, &existing, &mut errors);
    let option_header = header_map(option_rows.first().map(|(_, row)| row.as_slice()).unwrap_or(&[]));
    let option_drafts = if option_rows.is_empty() {
        Vec::new()
    } else {
        parse_options(&option_header, &option_rows, &mut errors)
    };
    if !errors.is_empty() {
        return Err(sheet_errors(errors));
    }
    check_item_plan(&drafts, &option_drafts, &existing, &variants, &mut errors);
    if !errors.is_empty() {
        return Err(sheet_errors(errors));
    }

    let mut tx = pool.begin().await?;
    let mut created = 0_i64;
    let mut updated = 0_i64;
    let mut id_of_row: HashMap<u32, i32> = HashMap::new();
    let mut barcode_of_new: HashMap<String, u32> = HashMap::new();
    for draft in &drafts {
        if let Some(id) = draft.id {
            sqlx::query(
                r#"
                UPDATE items
                SET section_id = $1, category_id = $2, name = $3, rate = $4,
                    prompt = $5, base_rate = $6, scale = $7, avg_cost = COALESCE($8, avg_cost), barcode = NULL
                WHERE id = $9 AND active
                "#,
            )
            .bind(draft.section_id)
            .bind(draft.category_id)
            .bind(&draft.name)
            .bind(draft.rate)
            .bind(&draft.prompt)
            .bind(draft.base_rate)
            .bind(draft.scale)
            .bind(draft.cost)
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(catalog_error)?;
            id_of_row.insert(draft.row, id);
            updated += 1;
        } else {
            let sort_order = sqlx::query_scalar::<_, i32>(
                "SELECT COALESCE(MAX(sort_order), 0) + 1 FROM items WHERE section_id = $1",
            )
            .bind(draft.section_id)
            .fetch_one(&mut *tx)
            .await?;
            let id = sqlx::query_scalar::<_, i32>(
                r#"
                INSERT INTO items (
                    section_id, category_id, barcode, name, rate, stock, prompt, base_rate, scale, avg_cost, sort_order
                )
                VALUES ($1, $2, NULL, $3, $4, 0, $5, $6, $7, $8, $9)
                RETURNING id
                "#,
            )
            .bind(draft.section_id)
            .bind(draft.category_id)
            .bind(&draft.name)
            .bind(draft.rate)
            .bind(&draft.prompt)
            .bind(draft.base_rate)
            .bind(draft.scale)
            .bind(draft.cost.unwrap_or(0))
            .bind(sort_order)
            .fetch_one(&mut *tx)
            .await
            .map_err(catalog_error)?;
            id_of_row.insert(draft.row, id);
            if let Some(code) = &draft.barcode {
                barcode_of_new.insert(code.clone(), draft.row);
            }
            created += 1;
        }
    }
    for draft in &drafts {
        let id = id_of_row[&draft.row];
        sqlx::query("UPDATE items SET barcode = $1 WHERE id = $2")
            .bind(&draft.barcode)
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(catalog_error)?;
    }

    let mut options_saved = 0_i64;
    for draft in &option_drafts {
        let item_id = resolve_option_item(draft, &id_of_row, &drafts, &barcode_of_new, &existing)?;
        if draft.remove {
            let deleted = sqlx::query("DELETE FROM item_variants WHERE id = $1 AND item_id = $2")
                .bind(draft.option_id)
                .bind(item_id)
                .execute(&mut *tx)
                .await?;
            if deleted.rows_affected() == 0 {
                return Err(AppError::BadRequest(format!(
                    "Options row {}: that option is not on this item.",
                    draft.row
                )));
            }
            options_saved += 1;
            continue;
        }
        if let Some(option_id) = draft.option_id {
            sqlx::query("UPDATE item_variants SET barcode = NULL WHERE id = $1 AND item_id = $2")
                .bind(option_id)
                .bind(item_id)
                .execute(&mut *tx)
                .await?;
            let changed = sqlx::query(
                r#"
                UPDATE item_variants
                SET name = $1, rate = $2, units = $3, barcode = $4
                WHERE id = $5 AND item_id = $6
                "#,
            )
            .bind(&draft.name)
            .bind(draft.rate)
            .bind(draft.units)
            .bind(&draft.barcode)
            .bind(option_id)
            .bind(item_id)
            .execute(&mut *tx)
            .await
            .map_err(catalog_error)?;
            if changed.rows_affected() == 0 {
                return Err(AppError::BadRequest(format!(
                    "Options row {}: that option is not on this item.",
                    draft.row
                )));
            }
        } else {
            let sort_order = sqlx::query_scalar::<_, i32>(
                "SELECT COALESCE(MAX(sort_order), 0) + 1 FROM item_variants WHERE item_id = $1",
            )
            .bind(item_id)
            .fetch_one(&mut *tx)
            .await?;
            sqlx::query(
                r#"
                INSERT INTO item_variants (item_id, name, rate, units, barcode, sort_order)
                VALUES ($1, $2, $3, $4, $5, $6)
                "#,
            )
            .bind(item_id)
            .bind(&draft.name)
            .bind(draft.rate)
            .bind(draft.units)
            .bind(&draft.barcode)
            .bind(sort_order)
            .execute(&mut *tx)
            .await
            .map_err(catalog_error)?;
        }
        options_saved += 1;
    }

    let mut counted = 0_i64;
    for draft in &drafts {
        let id = id_of_row[&draft.row];
        let target = if draft.id.is_some() {
            draft.counted
        } else {
            draft.counted.or(draft.opening)
        };
        let Some(target) = target else { continue };
        let on_hand = sqlx::query_scalar::<_, i32>("SELECT stock FROM items WHERE id = $1 FOR UPDATE")
            .bind(id)
            .fetch_one(&mut *tx)
            .await?;
        let delta = target - on_hand;
        if delta == 0 {
            continue;
        }
        let note = if draft.id.is_some() {
            "Excel stock count"
        } else {
            "Opening stock"
        };
        apply_delta(&mut tx, id, delta, note).await?;
        counted += 1;
    }
    tx.commit().await?;
    Ok(ItemImport {
        created,
        updated,
        options: options_saved,
        counted,
        pos: load_pos(pool, shop).await?,
    })
}

pub async fn import_movements(
    pool: &PgPool,
    shop: &ShopConfig,
    bytes: &[u8],
) -> Result<MovementImport, AppError> {
    let mut book = open_book(bytes)?;
    let Some(range) = named_range(&mut book, "Movements")? else {
        return Err(AppError::BadRequest(
            "The workbook needs a Movements sheet. Export a fresh file to get the columns.".into(),
        ));
    };
    let rows = read_rows(&range);
    if rows.len() > MAX_ROWS {
        return Err(AppError::BadRequest("A sheet can hold up to 5000 rows.".into()));
    }
    let header = header_map(rows.first().map(|(_, row)| row.as_slice()).unwrap_or(&[]));
    let items = sqlx::query_as::<_, LoadedItem>(
        "SELECT id, barcode, name FROM items WHERE active",
    )
    .fetch_all(pool)
    .await?;
    let variant_owners = sqlx::query_as::<_, (Option<String>, i32)>(
        r#"
        SELECT v.barcode, v.item_id
        FROM item_variants v
        JOIN items i ON i.id = v.item_id
        WHERE i.active AND v.barcode IS NOT NULL
        "#,
    )
    .fetch_all(pool)
    .await?;
    let mut errors = Vec::new();
    let drafts = parse_movements(&header, &rows, &items, &variant_owners, &mut errors);
    if !errors.is_empty() {
        return Err(sheet_errors(errors));
    }

    let mut tx = pool.begin().await?;
    let mut on_hand: HashMap<i32, i32> = HashMap::new();
    let mut ids: Vec<i32> = drafts.iter().map(|draft| draft.item_id).collect();
    ids.sort_unstable();
    ids.dedup();
    if !ids.is_empty() {
        let locked = sqlx::query_as::<_, (i32, i32)>(
            "SELECT id, stock FROM items WHERE id = ANY($1) AND active ORDER BY id FOR UPDATE",
        )
        .bind(&ids)
        .fetch_all(&mut *tx)
        .await?;
        if locked.len() != ids.len() {
            return Err(AppError::BadRequest(
                "One of those items is no longer in the catalog.".into(),
            ));
        }
        on_hand.extend(locked);
    }
    for draft in &drafts {
        let stock = on_hand.get_mut(&draft.item_id).expect("locked");
        let next = i64::from(*stock) + i64::from(draft.qty);
        if next < 0 {
            return Err(AppError::BadRequest(format!(
                "Movements row {}: only {stock} of {} are on hand.",
                draft.row, draft.item_name
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
    for draft in &drafts {
        apply_delta(&mut tx, draft.item_id, draft.qty, &draft.note).await?;
    }
    let applied = drafts.len() as i64;
    tx.commit().await?;
    Ok(MovementImport {
        applied,
        pos: load_pos(pool, shop).await?,
    })
}

pub async fn export_categories(pool: &PgPool) -> Result<Vec<u8>, AppError> {
    let rows = sqlx::query_as::<_, CategorySheetRow>(
        "SELECT id, name, sort_order FROM categories ORDER BY sort_order, id",
    )
    .fetch_all(pool)
    .await?;

    let mut workbook = Workbook::new();
    let header = Format::new().set_bold().set_background_color(Color::RGB(0xE2E8F0));
    let wrap = Format::new().set_text_wrap();
    let sheet = workbook.add_worksheet();
    sheet.set_name("Categories").sheet()?;
    for (col, name) in ["id", "name", "remove"].iter().enumerate() {
        sheet.write_string_with_format(0, col as u16, *name, &header).sheet()?;
    }
    for (index, row) in rows.iter().enumerate() {
        let line = (index + 1) as u32;
        sheet.write_number(line, 0, row.id as f64).sheet()?;
        sheet.write_string(line, 1, &row.name).sheet()?;
    }
    sheet.set_freeze_panes(1, 0).sheet()?;
    set_widths(sheet, &[10.0, 28.0, 12.0])?;

    let guide = workbook.add_worksheet();
    guide.set_name("Guide").sheet()?;
    let lines = [
        "One row per category. Leave id blank to add a category.",
        "An id updates that category. A name that already exists, with a blank id, updates that category.",
        "The row order is the catalog order. Categories you leave out stay after the ones in this sheet.",
        "Put yes in remove to delete that category. Move its items first. Keep at least one category.",
        "Names are unique, ignoring letter case, and can be up to 40 characters.",
    ];
    for (index, line) in lines.iter().enumerate() {
        guide.write_string_with_format(index as u32, 0, *line, &wrap).sheet()?;
    }
    guide.set_column_width(0, 110).sheet()?;
    workbook.save_to_buffer().sheet()
}

pub async fn import_categories(
    pool: &PgPool,
    shop: &ShopConfig,
    bytes: &[u8],
) -> Result<CategoryImport, AppError> {
    let mut book = open_book(bytes)?;
    let Some(range) = named_range(&mut book, "Categories")? else {
        return Err(AppError::BadRequest(
            "The workbook needs a Categories sheet. Export a fresh file to get the columns.".into(),
        ));
    };
    let rows = read_rows(&range);
    if rows.len() > MAX_ROWS {
        return Err(AppError::BadRequest("A sheet can hold up to 5000 rows.".into()));
    }
    let header = header_map(rows.first().map(|(_, row)| row.as_slice()).unwrap_or(&[]));
    let mut existing = sqlx::query_as::<_, CategorySheetRow>(
        "SELECT id, name, sort_order FROM categories ORDER BY sort_order, id",
    )
    .fetch_all(pool)
    .await?;
    existing.sort_by_key(|category| (category.sort_order, category.id));
    let mut errors = Vec::new();
    let mut drafts = parse_categories(&header, &rows, &mut errors);
    if errors.is_empty() {
        bind_categories(&mut drafts, &existing, &mut errors);
    }
    if !errors.is_empty() {
        return Err(sheet_errors(errors));
    }
    if drafts.is_empty() {
        return Ok(CategoryImport {
            created: 0,
            updated: 0,
            removed: 0,
            reordered: false,
            pos: load_pos(pool, shop).await?,
        });
    }

    let created = drafts.iter().filter(|draft| draft.id.is_none()).count() as i64;
    let updated = drafts
        .iter()
        .filter(|draft| !draft.remove)
        .filter(|draft| {
            draft.id.is_some_and(|id| {
                existing
                    .iter()
                    .find(|category| category.id == id)
                    .is_some_and(|category| category.name != draft.name)
            })
        })
        .count() as i64;
    let removed = drafts.iter().filter(|draft| draft.remove).count() as i64;
    let mut next_order: Vec<i32> = drafts.iter().filter(|draft| !draft.remove).filter_map(|draft| draft.id).collect();
    let listed: HashSet<i32> = next_order.iter().copied().collect();
    let removed_ids: HashSet<i32> = drafts.iter().filter(|draft| draft.remove).filter_map(|draft| draft.id).collect();
    for category in &existing {
        if !listed.contains(&category.id) && !removed_ids.contains(&category.id) {
            next_order.push(category.id);
        }
    }
    let previous_order: Vec<i32> = existing
        .iter()
        .filter(|category| !removed_ids.contains(&category.id))
        .map(|category| category.id)
        .collect();
    let reordered = previous_order != next_order;

    let mut tx = pool.begin().await?;
    for draft in drafts.iter().filter(|draft| draft.id.is_some()) {
        sqlx::query("UPDATE categories SET name = $1 WHERE id = $2")
            .bind(format!("\u{1}{}", draft.id.unwrap()))
            .bind(draft.id)
            .execute(&mut *tx)
            .await
            .map_err(category_sheet_error)?;
    }
    for draft in drafts.iter().filter(|draft| draft.remove) {
        let items = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM items WHERE category_id = $1")
            .bind(draft.id)
            .fetch_one(&mut *tx)
            .await?;
        if items > 0 {
            return Err(AppError::BadRequest(format!(
                "Categories row {}: move this category's items before removing it.",
                draft.row
            )));
        }
        let deleted = sqlx::query("DELETE FROM categories WHERE id = $1")
            .bind(draft.id)
            .execute(&mut *tx)
            .await?;
        if deleted.rows_affected() == 0 {
            return Err(AppError::BadRequest(format!(
                "Categories row {}: that category is not in the catalog.",
                draft.row
            )));
        }
    }
    for draft in drafts.iter_mut().filter(|draft| draft.id.is_none()) {
        let id = sqlx::query_scalar::<_, i32>(
            "INSERT INTO categories (name, sort_order) VALUES ($1, 0) RETURNING id",
        )
        .bind(&draft.name)
        .fetch_one(&mut *tx)
        .await
        .map_err(category_sheet_error)?;
        draft.id = Some(id);
    }
    for draft in drafts.iter().filter(|draft| !draft.remove) {
        sqlx::query("UPDATE categories SET name = $1 WHERE id = $2")
            .bind(&draft.name)
            .bind(draft.id)
            .execute(&mut *tx)
            .await
            .map_err(category_sheet_error)?;
    }
    let mut ordered: Vec<i32> = drafts.iter().filter(|draft| !draft.remove).filter_map(|draft| draft.id).collect();
    let listed: HashSet<i32> = ordered.iter().copied().collect();
    for category in &existing {
        if !listed.contains(&category.id) && !removed_ids.contains(&category.id) {
            ordered.push(category.id);
        }
    }
    for (index, id) in ordered.iter().enumerate() {
        sqlx::query("UPDATE categories SET sort_order = $1 WHERE id = $2")
            .bind(index as i32)
            .bind(id)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(CategoryImport {
        created,
        updated,
        removed,
        reordered,
        pos: load_pos(pool, shop).await?,
    })
}

async fn apply_delta(
    tx: &mut Transaction<'_, Postgres>,
    item_id: i32,
    delta: i32,
    note: &str,
) -> Result<(), AppError> {
    let inbound = delta > 0;
    let reason = movement_reason(inbound, note);
    sqlx::query("UPDATE items SET stock = stock + $1 WHERE id = $2")
        .bind(delta)
        .bind(item_id)
        .execute(&mut **tx)
        .await?;
    sqlx::query("INSERT INTO stock_movements (item_id, qty, reason) VALUES ($1, $2, $3)")
        .bind(item_id)
        .bind(delta)
        .bind(reason)
        .execute(&mut **tx)
        .await?;
    Ok(())
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

fn parse_items(
    header: &HashMap<String, usize>,
    rows: &[(u32, Vec<Data>)],
    sections: &[SectionRef],
    categories: &[CategoryRef],
    existing: &[LoadedItem],
    errors: &mut Vec<String>,
) -> Vec<ItemDraft> {
    if rows.is_empty() {
        return Vec::new();
    }
    let id_col = column(header, &["id"]);
    let barcode_col = column(header, &["barcode", "code"]);
    let name_col = column(header, &["name", "item", "item_name"]);
    let section_col = column(header, &["section", "head", "business_head"]);
    let category_col = column(header, &["category"]);
    let rate_col = column(header, &["rate", "sale_rate", "price"]);
    let cost_col = column(header, &["cost", "avg_cost", "average_cost"]);
    let prompt_col = column(header, &["prompt", "ask"]);
    let base_col = column(header, &["base_rate", "base"]);
    let scale_col = column(header, &["scale", "stock_per_1"]);
    let stock_col = column(header, &["stock"]);
    let counted_col = column(header, &["counted", "count"]);
    if name_col.is_none() || section_col.is_none() || category_col.is_none() || rate_col.is_none() {
        errors.push("The Items sheet needs name, section, category, and rate columns.".into());
        return Vec::new();
    }
    let mut drafts = Vec::new();
    for (excel_row, row) in rows.iter().skip(1) {
        if errors.len() >= 20 {
            break;
        }
        let name = cell_text(row, name_col);
        let barcode = cell_text(row, barcode_col);
        let id_text = cell_text(row, id_col);
        let counted_text = cell_text(row, counted_col);
        let stock_text = cell_text(row, stock_col);
        if name.is_empty() && barcode.is_empty() && id_text.is_empty() && counted_text.is_empty() {
            continue;
        }
        let label = format!("Items row {excel_row}");
        let id = match whole(&id_text, "id") {
            Ok(value) => value,
            Err(message) => {
                errors.push(format!("{label}: {message}"));
                continue;
            }
        };
        if name.is_empty() {
            errors.push(format!("{label}: enter an item name."));
            continue;
        }
        if name.chars().count() > 200 {
            errors.push(format!("{label}: keep the name to 200 characters."));
            continue;
        }
        let section_name = cell_text(row, section_col);
        let Some(section_id) = find_section(sections, &section_name) else {
            errors.push(format!("{label}: no business head named {section_name}."));
            continue;
        };
        let category_name = cell_text(row, category_col);
        let Some(category_id) = categories
            .iter()
            .find(|category| category.name.eq_ignore_ascii_case(category_name.trim()))
            .map(|category| category.id)
        else {
            errors.push(format!("{label}: no category named {category_name}."));
            continue;
        };
        let cost = match cost_cents(&cell_text(row, cost_col)) {
            Ok(value) => value,
            Err(message) => {
                errors.push(format!("{label}: {message}"));
                continue;
            }
        };
        let rate = match required_whole(&cell_text(row, rate_col), "rate") {
            Ok(value) if value >= 1 => value,
            Ok(_) => {
                errors.push(format!("{label}: enter a shelf rate above 0."));
                continue;
            }
            Err(message) => {
                errors.push(format!("{label}: {message}"));
                continue;
            }
        };
        let prompt = match normalize_prompt(&cell_text(row, prompt_col)) {
            Ok(value) => value,
            Err(message) => {
                errors.push(format!("{label}: {message}"));
                continue;
            }
        };
        let base_rate = match whole(&cell_text(row, base_col), "base rate") {
            Ok(value) => value,
            Err(message) => {
                errors.push(format!("{label}: {message}"));
                continue;
            }
        };
        let scale = match whole(&cell_text(row, scale_col), "scale") {
            Ok(value) => value.unwrap_or(1),
            Err(message) => {
                errors.push(format!("{label}: {message}"));
                continue;
            }
        };
        let (base_rate, scale) = if prompt == "none" {
            (None, 1)
        } else if base_rate.unwrap_or(0) < 1 || scale < 1 {
            errors.push(format!("{label}: set a base rate above 0 and a scale of at least 1."));
            continue;
        } else {
            (base_rate, scale)
        };
        let counted = match whole(&counted_text, "counted") {
            Ok(value) => value,
            Err(message) => {
                errors.push(format!("{label}: {message}"));
                continue;
            }
        };
        let opening = match whole(&stock_text, "stock") {
            Ok(value) => value,
            Err(message) => {
                errors.push(format!("{label}: {message}"));
                continue;
            }
        };
        if counted.is_some_and(|value| value < 0) || (id.is_none() && opening.is_some_and(|value| value < 0)) {
            errors.push(format!("{label}: stock cannot be negative."));
            continue;
        }
        if id.is_some_and(|value| !existing.iter().any(|item| item.id == value)) {
            errors.push(format!("{label}: that item is not in the catalog."));
            continue;
        }
        let barcode = clean_code(&barcode);
        if barcode.as_ref().is_some_and(|code| code.chars().count() > 64) {
            errors.push(format!("{label}: keep the barcode to 64 characters."));
            continue;
        }
        drafts.push(ItemDraft {
            row: *excel_row,
            id,
            barcode,
            name,
            section_id,
            category_id,
            rate,
            cost,
            prompt,
            base_rate,
            scale,
            counted,
            opening,
        });
    }
    drafts
}

fn parse_options(
    header: &HashMap<String, usize>,
    rows: &[(u32, Vec<Data>)],
    errors: &mut Vec<String>,
) -> Vec<OptionDraft> {
    if rows.is_empty() {
        return Vec::new();
    }
    let item_col = column(header, &["item_id"]);
    let item_code_col = column(header, &["item_barcode"]);
    let option_col = column(header, &["option_id", "variant_id"]);
    let name_col = column(header, &["name", "option", "option_name"]);
    let barcode_col = column(header, &["barcode", "option_barcode"]);
    let rate_col = column(header, &["rate"]);
    let units_col = column(header, &["units", "stock_units"]);
    let remove_col = column(header, &["remove", "delete"]);
    if item_col.is_none() && item_code_col.is_none() {
        errors.push("The Options sheet needs an item_id or item_barcode column.".into());
        return Vec::new();
    }
    let mut drafts = Vec::new();
    for (excel_row, row) in rows.iter().skip(1) {
        if errors.len() >= 20 {
            break;
        }
        let item_id_text = cell_text(row, item_col);
        let item_barcode = clean_code(&cell_text(row, item_code_col));
        let option_text = cell_text(row, option_col);
        let name = cell_text(row, name_col);
        let barcode = cell_text(row, barcode_col);
        let remove = is_yes(&cell_text(row, remove_col));
        if item_id_text.is_empty() && item_barcode.is_none() && option_text.is_empty() && name.is_empty() {
            continue;
        }
        let label = format!("Options row {excel_row}");
        let item_id = match whole(&item_id_text, "item id") {
            Ok(value) => value,
            Err(message) => {
                errors.push(format!("{label}: {message}"));
                continue;
            }
        };
        let option_id = match whole(&option_text, "option id") {
            Ok(value) => value,
            Err(message) => {
                errors.push(format!("{label}: {message}"));
                continue;
            }
        };
        if item_id.is_none() && item_barcode.is_none() {
            errors.push(format!("{label}: say which item this option belongs to."));
            continue;
        }
        if remove && option_id.is_none() {
            errors.push(format!("{label}: say which option to remove."));
            continue;
        }
        let (rate, units, barcode) = if remove {
            (1, 1, None)
        } else {
            if name.is_empty() {
                errors.push(format!("{label}: name the option."));
                continue;
            }
            let rate = match required_whole(&cell_text(row, rate_col), "rate") {
                Ok(value) if value >= 1 => value,
                _ => {
                    errors.push(format!("{label}: enter a rate above 0."));
                    continue;
                }
            };
            let units = match required_whole(&cell_text(row, units_col), "units") {
                Ok(value) if value >= 1 => value,
                _ => {
                    errors.push(format!("{label}: units must be at least 1."));
                    continue;
                }
            };
            let barcode = clean_code(&barcode);
            if barcode.as_ref().is_some_and(|code| code.chars().count() > 64) {
                errors.push(format!("{label}: keep the barcode to 64 characters."));
                continue;
            }
            (rate, units, barcode)
        };
        drafts.push(OptionDraft {
            row: *excel_row,
            item_id,
            item_barcode,
            option_id,
            name,
            barcode,
            rate,
            units,
            remove,
        });
    }
    drafts
}

fn parse_categories(
    header: &HashMap<String, usize>,
    rows: &[(u32, Vec<Data>)],
    errors: &mut Vec<String>,
) -> Vec<CategoryDraft> {
    if rows.is_empty() {
        return Vec::new();
    }
    let id_col = column(header, &["id"]);
    let name_col = column(header, &["name", "category"]);
    let remove_col = column(header, &["remove", "delete"]);
    if name_col.is_none() {
        errors.push("The Categories sheet needs a name column.".into());
        return Vec::new();
    }
    let mut drafts = Vec::new();
    for (excel_row, row) in rows.iter().skip(1) {
        if errors.len() >= 20 {
            break;
        }
        let id_text = cell_text(row, id_col);
        let name = cell_text(row, name_col);
        let remove = is_yes(&cell_text(row, remove_col));
        if id_text.is_empty() && name.is_empty() && !remove {
            continue;
        }
        let label = format!("Categories row {excel_row}");
        let id = match whole(&id_text, "id") {
            Ok(value) => value,
            Err(message) => {
                errors.push(format!("{label}: {message}"));
                continue;
            }
        };
        if !remove && name.is_empty() {
            errors.push(format!("{label}: enter a category name."));
            continue;
        }
        if name.chars().count() > 40 {
            errors.push(format!("{label}: keep the category name to 40 characters."));
            continue;
        }
        drafts.push(CategoryDraft {
            row: *excel_row,
            id,
            name,
            remove,
        });
    }
    drafts
}

fn bind_categories(drafts: &mut [CategoryDraft], existing: &[CategorySheetRow], errors: &mut Vec<String>) {
    let mut seen_ids: HashMap<i32, u32> = HashMap::new();
    for draft in drafts.iter() {
        if errors.len() >= 20 {
            return;
        }
        let Some(id) = draft.id else { continue };
        let label = format!("Categories row {}", draft.row);
        if !existing.iter().any(|category| category.id == id) {
            errors.push(format!("{label}: that category is not in the catalog."));
            continue;
        }
        if draft.remove {
            if let Some(category) = existing.iter().find(|category| category.id == id) {
                if !draft.name.is_empty() && !category.name.eq_ignore_ascii_case(&draft.name) {
                    errors.push(format!("{label}: that name does not belong to this category."));
                    continue;
                }
            }
        }
        if let Some(previous) = seen_ids.insert(id, draft.row) {
            errors.push(format!("{label}: category {id} is also on row {previous}."));
        }
    }
    if !errors.is_empty() {
        return;
    }
    let mut bindings = Vec::new();
    for index in 0..drafts.len() {
        if errors.len() >= 20 {
            return;
        }
        if drafts[index].id.is_some() {
            continue;
        }
        let row = drafts[index].row;
        let name = drafts[index].name.clone();
        let remove = drafts[index].remove;
        let label = format!("Categories row {row}");
        let Some(category_id) = existing
            .iter()
            .find(|category| category.name.eq_ignore_ascii_case(&name))
            .map(|category| category.id)
        else {
            if remove {
                errors.push(format!("{label}: say which category to remove."));
            }
            continue;
        };
        if let Some(previous) = seen_ids.get(&category_id).copied() {
            let owner_keeps = drafts.iter().any(|other| {
                other.row == previous && !other.remove && other.name.eq_ignore_ascii_case(&name)
            });
            if remove || owner_keeps {
                errors.push(format!(
                    "{label}: category {category_id} is also on row {previous}."
                ));
            }
            continue;
        }
        seen_ids.insert(category_id, row);
        bindings.push((index, category_id));
    }
    for (index, id) in bindings {
        drafts[index].id = Some(id);
    }
    if !errors.is_empty() {
        return;
    }
    let touched: HashSet<i32> = drafts.iter().filter_map(|draft| draft.id).collect();
    let mut seen_names: HashMap<String, u32> = HashMap::new();
    for draft in drafts.iter() {
        if errors.len() >= 20 {
            return;
        }
        if draft.remove {
            continue;
        }
        let label = format!("Categories row {}", draft.row);
        let key = draft.name.to_lowercase();
        if let Some(previous) = seen_names.insert(key, draft.row) {
            errors.push(format!("{label}: that name is also on row {previous}."));
            continue;
        }
        if existing.iter().any(|category| {
            category.name.eq_ignore_ascii_case(&draft.name) && !touched.contains(&category.id)
        }) {
            errors.push(format!("{label}: that category name is already used."));
        }
    }
    let removed = drafts.iter().filter(|draft| draft.remove).count();
    let created = drafts.iter().filter(|draft| draft.id.is_none()).count();
    if existing.len() + created < removed + 1 {
        errors.push("Keep at least one category.".into());
    }
}

fn parse_movements(
    header: &HashMap<String, usize>,
    rows: &[(u32, Vec<Data>)],
    items: &[LoadedItem],
    variant_owners: &[(Option<String>, i32)],
    errors: &mut Vec<String>,
) -> Vec<MoveDraft> {
    if rows.is_empty() {
        return Vec::new();
    }
    let id_col = column(header, &["id", "movement_id"]);
    let barcode_col = column(header, &["barcode", "code"]);
    let item_col = column(header, &["item", "name", "item_name"]);
    let qty_col = column(header, &["qty", "quantity"]);
    let direction_col = column(header, &["direction", "type"]);
    let note_col = column(header, &["note", "reason"]);
    if qty_col.is_none() || (barcode_col.is_none() && item_col.is_none()) {
        errors.push("The Movements sheet needs a barcode or item column, and a qty column.".into());
        return Vec::new();
    }
    let mut drafts = Vec::new();
    for (excel_row, row) in rows.iter().skip(1) {
        if errors.len() >= 20 {
            break;
        }
        let id_text = cell_text(row, id_col);
        let barcode = clean_code(&cell_text(row, barcode_col));
        let item_name = cell_text(row, item_col);
        let qty_text = cell_text(row, qty_col);
        let direction = cell_text(row, direction_col).to_lowercase();
        let note = cell_text(row, note_col);
        if id_text.is_empty() && barcode.is_none() && item_name.is_empty() && qty_text.is_empty() {
            continue;
        }
        let label = format!("Movements row {excel_row}");
        match whole(&id_text, "id") {
            Ok(Some(_)) => continue,
            Ok(None) => {}
            Err(message) => {
                errors.push(format!("{label}: {message}"));
                continue;
            }
        }
        let qty = match required_whole(&qty_text, "qty") {
            Ok(value) => value,
            Err(message) => {
                errors.push(format!("{label}: {message}"));
                continue;
            }
        };
        let signed = match direction.as_str() {
            "" => qty,
            "in" | "stock in" | "add" if qty > 0 => qty,
            "out" | "stock out" | "deduct" if qty > 0 => -qty,
            "in" | "out" | "stock in" | "stock out" | "add" | "deduct" => {
                errors.push(format!("{label}: use a positive quantity with direction."));
                continue;
            }
            _ => {
                errors.push(format!("{label}: direction is in or out."));
                continue;
            }
        };
        if signed == 0 || signed.abs() > 1_000_000 {
            errors.push(format!("{label}: enter a quantity from 1 to 1000000."));
            continue;
        }
        if note.chars().count() > 80 {
            errors.push(format!("{label}: keep the note to 80 characters."));
            continue;
        }
        let Some((item_id, name)) = find_movement_item(barcode.as_deref(), &item_name, items, variant_owners) else {
            errors.push(format!("{label}: that item is not in the catalog."));
            continue;
        };
        drafts.push(MoveDraft {
            row: *excel_row,
            item_id,
            item_name: name,
            qty: signed,
            note,
        });
    }
    drafts
}

fn find_movement_item(
    barcode: Option<&str>,
    item_name: &str,
    items: &[LoadedItem],
    variant_owners: &[(Option<String>, i32)],
) -> Option<(i32, String)> {
    if let Some(code) = barcode {
        if let Some(item) = items.iter().find(|item| item.barcode.as_deref() == Some(code)) {
            return Some((item.id, item.name.clone()));
        }
        if let Some((_, item_id)) = variant_owners.iter().find(|(owned, _)| owned.as_deref() == Some(code)) {
            let name = items
                .iter()
                .find(|item| item.id == *item_id)
                .map(|item| item.name.clone())
                .unwrap_or_else(|| code.to_string());
            return Some((*item_id, name));
        }
        return None;
    }
    let needle = item_name.trim();
    if needle.is_empty() {
        return None;
    }
    let hits: Vec<_> = items
        .iter()
        .filter(|item| item.name.eq_ignore_ascii_case(needle))
        .collect();
    if hits.len() == 1 {
        Some((hits[0].id, hits[0].name.clone()))
    } else {
        None
    }
}

fn check_item_plan(
    drafts: &[ItemDraft],
    options: &[OptionDraft],
    existing: &[LoadedItem],
    variants: &[LoadedVariant],
    errors: &mut Vec<String>,
) {
    let mut seen_ids = HashMap::<i32, u32>::new();
    let mut seen_codes = HashMap::<String, String>::new();
    for item in existing {
        if drafts.iter().any(|draft| draft.id == Some(item.id)) {
            continue;
        }
        if let Some(code) = &item.barcode {
            seen_codes.insert(code.clone(), format!("item {}", item.id));
        }
    }
    for variant in variants {
        let removed = options.iter().any(|draft| draft.remove && draft.option_id == Some(variant.id));
        let replaced = options.iter().any(|draft| draft.option_id == Some(variant.id));
        if removed || replaced {
            continue;
        }
        if let Some(code) = &variant.barcode {
            seen_codes.insert(code.clone(), format!("option {}", variant.id));
        }
    }
    for draft in drafts {
        if let Some(id) = draft.id {
            if let Some(previous) = seen_ids.insert(id, draft.row) {
                errors.push(format!(
                    "Items row {}: item {id} is also on row {previous}.",
                    draft.row
                ));
            }
        }
        if let Some(code) = &draft.barcode {
            if let Some(owner) = seen_codes.insert(code.clone(), format!("Items row {}", draft.row)) {
                errors.push(format!(
                    "Items row {}: barcode {code} is already used by {owner}.",
                    draft.row
                ));
            }
        }
    }
    for draft in options {
        if draft.remove {
            continue;
        }
        if let Some(code) = &draft.barcode {
            if let Some(owner) = seen_codes.insert(code.clone(), format!("Options row {}", draft.row)) {
                errors.push(format!(
                    "Options row {}: barcode {code} is already used by {owner}.",
                    draft.row
                ));
            }
        }
        if let Some(item_id) = draft.item_id {
            let names: Vec<_> = options
                .iter()
                .filter(|other| !other.remove && other.item_id == Some(item_id))
                .map(|other| other.name.to_lowercase())
                .collect();
            if names.iter().filter(|name| *name == &draft.name.to_lowercase()).count() > 1 {
                errors.push(format!(
                    "Options row {}: each option needs its own name.",
                    draft.row
                ));
            }
        }
    }
    for draft in drafts {
        if draft.prompt == "none" {
            continue;
        }
        let removed: Vec<i32> = options.iter().filter_map(|option| option.remove.then_some(option.option_id).flatten()).collect();
        let kept = draft.id.is_some_and(|id| {
            variants
                .iter()
                .any(|variant| variant.item_id == id && !removed.contains(&variant.id))
        });
        let added = options.iter().any(|option| {
            !option.remove
                && ((draft.id.is_some() && option.item_id == draft.id)
                    || (draft.barcode.is_some() && option.item_barcode == draft.barcode))
        });
        if kept || added {
            errors.push(format!(
                "Items row {}: an item with options cannot also ask for a quantity or a price.",
                draft.row
            ));
        }
    }
}

fn resolve_option_item(
    draft: &OptionDraft,
    id_of_row: &HashMap<u32, i32>,
    items: &[ItemDraft],
    barcode_of_new: &HashMap<String, u32>,
    existing: &[LoadedItem],
) -> Result<i32, AppError> {
    if let Some(item_id) = draft.item_id {
        let known = existing.iter().any(|item| item.id == item_id)
            || id_of_row.values().any(|id| *id == item_id);
        if !known {
            return Err(AppError::BadRequest(format!(
                "Options row {}: that item is not in the catalog.",
                draft.row
            )));
        }
        if let Some(code) = &draft.item_barcode {
            let matches = existing.iter().any(|item| item.id == item_id && item.barcode.as_ref() == Some(code))
                || items.iter().any(|item| item.id == Some(item_id) && item.barcode.as_ref() == Some(code))
                || items.iter().any(|item| {
                    id_of_row.get(&item.row) == Some(&item_id) && item.barcode.as_ref() == Some(code)
                });
            if !matches {
                return Err(AppError::BadRequest(format!(
                    "Options row {}: that barcode does not belong to item {item_id}.",
                    draft.row
                )));
            }
        }
        return Ok(item_id);
    }
    let Some(code) = &draft.item_barcode else {
        return Err(AppError::BadRequest(format!(
            "Options row {}: say which item this option belongs to.",
            draft.row
        )));
    };
    if let Some(row) = barcode_of_new.get(code) {
        return Ok(id_of_row[row]);
    }
    if let Some(item) = items.iter().find(|item| item.barcode.as_ref() == Some(code)) {
        return Ok(id_of_row[&item.row]);
    }
    if let Some(item) = existing.iter().find(|item| item.barcode.as_ref() == Some(code)) {
        return Ok(item.id);
    }
    Err(AppError::BadRequest(format!(
        "Options row {}: no item has barcode {code}.",
        draft.row
    )))
}

fn find_section(sections: &[SectionRef], label: &str) -> Option<i32> {
    let label = label.trim();
    if label.is_empty() {
        return None;
    }
    sections
        .iter()
        .find(|section| section.short_name.eq_ignore_ascii_case(label))
        .or_else(|| sections.iter().find(|section| section.name.eq_ignore_ascii_case(label)))
        .map(|section| section.id)
}

fn normalize_prompt(value: &str) -> Result<String, String> {
    match value.trim().to_lowercase().as_str() {
        "" | "none" | "off" => Ok("none".into()),
        "quantity" | "weight" => Ok("quantity".into()),
        "price" => Ok("price".into()),
        _ => Err("prompt is none, quantity, or price".into()),
    }
}

fn clean_code(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() { None } else { Some(value.to_string()) }
}

fn is_yes(value: &str) -> bool {
    matches!(value.trim().to_lowercase().as_str(), "yes" | "y" | "true" | "1" | "remove" | "delete")
}

fn whole(value: &str, label: &str) -> Result<Option<i32>, String> {
    if value.trim().is_empty() {
        return Ok(None);
    }
    value
        .trim()
        .parse::<i32>()
        .map(Some)
        .map_err(|_| format!("{label} must be a whole number"))
}

fn required_whole(value: &str, label: &str) -> Result<i32, String> {
    whole(value, label)?.ok_or_else(|| format!("{label} is required"))
}

fn sheet_errors(errors: Vec<String>) -> AppError {
    AppError::BadRequest(errors.into_iter().take(20).collect::<Vec<_>>().join("\n"))
}

fn category_sheet_error(err: sqlx::Error) -> AppError {
    let constraint = err
        .as_database_error()
        .and_then(|db| db.constraint().map(str::to_string));
    match constraint.as_deref() {
        Some("categories_name_key") | Some("categories_name_lower_idx") => {
            AppError::BadRequest("That category name is already used.".into())
        }
        _ => AppError::Db(err),
    }
}

fn catalog_error(err: sqlx::Error) -> AppError {
    let constraint = err
        .as_database_error()
        .and_then(|db| db.constraint().map(str::to_string));
    match constraint.as_deref() {
        Some("items_barcode_key") | Some("item_variants_barcode_key") => {
            AppError::BadRequest("That barcode is already used.".into())
        }
        Some("item_variants_item_id_name_key") => {
            AppError::BadRequest("Each option needs its own name.".into())
        }
        _ => AppError::Db(err),
    }
}

fn open_book(bytes: &[u8]) -> Result<Xlsx<Cursor<&[u8]>>, AppError> {
    if bytes.is_empty() {
        return Err(AppError::BadRequest("Choose an Excel workbook.".into()));
    }
    Xlsx::new(Cursor::new(bytes)).map_err(|_| {
        AppError::BadRequest("That file is not an Excel workbook (.xlsx).".into())
    })
}

fn named_range(book: &mut Xlsx<Cursor<&[u8]>>, name: &str) -> Result<Option<calamine::Range<Data>>, AppError> {
    let found = book
        .sheet_names()
        .into_iter()
        .find(|sheet| sheet.eq_ignore_ascii_case(name));
    let Some(found) = found else { return Ok(None) };
    book.worksheet_range(&found)
        .map(Some)
        .map_err(|_| AppError::BadRequest(format!("The {name} sheet could not be read.")))
}

fn read_rows(range: &calamine::Range<Data>) -> Vec<(u32, Vec<Data>)> {
    let start = range.start().map(|(row, _)| row).unwrap_or(0);
    range
        .rows()
        .enumerate()
        .map(|(index, row)| (start + index as u32 + 1, row.to_vec()))
        .collect()
}

fn header_map(row: &[Data]) -> HashMap<String, usize> {
    row.iter()
        .enumerate()
        .filter_map(|(index, cell)| {
            let name = cell_string(cell).trim().to_lowercase().replace(' ', "_");
            if name.is_empty() { None } else { Some((name, index)) }
        })
        .collect()
}

fn column(header: &HashMap<String, usize>, names: &[&str]) -> Option<usize> {
    names.iter().find_map(|name| header.get(*name).copied())
}

fn cost_text(cents: i64) -> String {
    let major = cents / 100;
    let frac = (cents % 100).abs();
    if frac == 0 {
        major.to_string()
    } else {
        format!("{major}.{frac:02}")
    }
}

fn cost_cents(text: &str) -> Result<Option<i64>, String> {
    let trimmed = text.trim().replace(',', "");
    if trimmed.is_empty() {
        return Ok(None);
    }
    if trimmed.starts_with('-') {
        return Err("cost cannot be negative.".into());
    }
    let (major, frac) = match trimmed.split_once('.') {
        Some((major, frac)) => (major, frac.trim_end_matches('0')),
        None => (trimmed.as_str(), ""),
    };
    if major.is_empty() || !major.chars().all(|ch| ch.is_ascii_digit()) || frac.len() > 2 || !frac.chars().all(|ch| ch.is_ascii_digit()) {
        return Err("cost uses up to 2 decimals.".into());
    }
    let major: i64 = major.parse().map_err(|_| "cost is too large.".to_string())?;
    if major > 100_000_000 {
        return Err("cost is too large.".into());
    }
    let frac_num: i64 = if frac.is_empty() {
        0
    } else {
        format!("{frac:0<2}").parse().unwrap_or(0)
    };
    Ok(Some(major * 100 + frac_num))
}

fn cell_text(row: &[Data], index: Option<usize>) -> String {
    index
        .and_then(|index| row.get(index))
        .map(cell_string)
        .unwrap_or_default()
}

fn cell_string(cell: &Data) -> String {
    match cell {
        Data::Empty => String::new(),
        Data::String(value) => value.trim().to_string(),
        Data::Float(value) if value.fract() == 0.0 && value.abs() < i64::MAX as f64 => {
            format!("{}", *value as i64)
        }
        Data::Float(value) => format!("{value}"),
        Data::Int(value) => value.to_string(),
        Data::Bool(value) => if *value { "yes".into() } else { String::new() },
        _ => String::new(),
    }
}

fn write_text(
    sheet: &mut rust_xlsxwriter::Worksheet,
    row: u32,
    col: u16,
    value: &str,
    format: &Format,
) -> Result<(), AppError> {
    sheet.write_string_with_format(row, col, value, format).sheet()?;
    Ok(())
}

fn set_widths(sheet: &mut rust_xlsxwriter::Worksheet, widths: &[f64]) -> Result<(), AppError> {
    for (col, width) in widths.iter().enumerate() {
        sheet.set_column_width(col as u16, *width).sheet()?;
    }
    Ok(())
}

trait SheetResult<T> {
    fn sheet(self) -> Result<T, AppError>;
}

impl<T> SheetResult<T> for Result<T, rust_xlsxwriter::XlsxError> {
    fn sheet(self) -> Result<T, AppError> {
        self.map_err(|_| AppError::BadRequest("The workbook could not be built.".into()))
    }
}

#[derive(sqlx::FromRow)]
struct ItemSheetRow {
    id: i32,
    barcode: Option<String>,
    name: String,
    section: String,
    category: String,
    rate: i32,
    avg_cost: i64,
    prompt: String,
    base_rate: Option<i32>,
    scale: i32,
    stock: i32,
}

#[derive(sqlx::FromRow)]
struct OptionSheetRow {
    item_id: i32,
    item_barcode: Option<String>,
    item_name: String,
    option_id: i32,
    name: String,
    barcode: Option<String>,
    rate: i32,
    units: i32,
}

#[derive(sqlx::FromRow)]
struct CategorySheetRow {
    id: i32,
    name: String,
    sort_order: i32,
}

#[derive(sqlx::FromRow)]
struct MovementSheetRow {
    id: i32,
    time_utc: String,
    barcode: Option<String>,
    item: String,
    section: String,
    qty: i32,
    reason: String,
    bill_no: Option<String>,
}
