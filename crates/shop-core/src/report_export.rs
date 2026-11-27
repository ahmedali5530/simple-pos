//! Downloadable sales / ledger / mix reports for a chosen date range.

use rust_xlsxwriter::{Color, Format, Workbook};
use sqlx::PgPool;

use crate::error::AppError;
use crate::ledger::{load_reports, product_mix, MixLine, Reports, StockMovement};
use crate::sheets::{set_widths, write_text, SheetResult};

const EXPORT_LIMIT: i64 = 10_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportKind {
    Sales,
    Ledger,
    Mix,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportFormat {
    Xlsx,
    Pdf,
}

impl ReportKind {
    pub fn parse(value: &str) -> Result<Self, AppError> {
        match value.trim().to_ascii_lowercase().as_str() {
            "sales" => Ok(Self::Sales),
            "ledger" => Ok(Self::Ledger),
            "mix" => Ok(Self::Mix),
            _ => Err(AppError::BadRequest(
                "Export kind must be sales, ledger, or mix.".into(),
            )),
        }
    }

    pub fn file_stem(self) -> &'static str {
        match self {
            Self::Sales => "sales-report",
            Self::Ledger => "stock-ledger",
            Self::Mix => "product-mix",
        }
    }
}

impl ReportFormat {
    pub fn parse(value: &str) -> Result<Self, AppError> {
        match value.trim().to_ascii_lowercase().as_str() {
            "xlsx" | "excel" => Ok(Self::Xlsx),
            "pdf" => Ok(Self::Pdf),
            _ => Err(AppError::BadRequest(
                "Export format must be xlsx or pdf.".into(),
            )),
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            Self::Xlsx => "xlsx",
            Self::Pdf => "pdf",
        }
    }

    pub fn content_type(self) -> &'static str {
        match self {
            Self::Xlsx => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
            Self::Pdf => "application/pdf",
        }
    }
}

pub async fn export_report(
    pool: &PgPool,
    kind: ReportKind,
    format: ReportFormat,
    from: Option<&str>,
    to: Option<&str>,
) -> Result<Vec<u8>, AppError> {
    match (kind, format) {
        (ReportKind::Sales, ReportFormat::Xlsx) => export_sales_xlsx(pool, from, to).await,
        (ReportKind::Sales, ReportFormat::Pdf) => export_sales_pdf(pool, from, to).await,
        (ReportKind::Ledger, ReportFormat::Xlsx) => export_ledger_xlsx(pool, from, to).await,
        (ReportKind::Ledger, ReportFormat::Pdf) => export_ledger_pdf(pool, from, to).await,
        (ReportKind::Mix, ReportFormat::Xlsx) => export_mix_xlsx(pool, from, to).await,
        (ReportKind::Mix, ReportFormat::Pdf) => export_mix_pdf(pool, from, to).await,
    }
}

async fn load_full_reports(
    pool: &PgPool,
    from: Option<&str>,
    to: Option<&str>,
) -> Result<Reports, AppError> {
    // Reuse the shared report loader, then replace truncated lists with a wider export pull.
    let mut report = load_reports(pool, from, to).await?;
    report.bills = load_export_bills(pool, from, to).await?;
    report.movements = load_export_movements(pool, from, to).await?;
    let mix = product_mix_export(pool, from, to).await?;
    report.mix = mix.lines;
    report.mix_truncated = mix.truncated;
    report.range_label = mix.label;
    report.bills_truncated = report.bill_count > report.bills.len() as i64;
    Ok(report)
}

async fn load_export_bills(
    pool: &PgPool,
    from: Option<&str>,
    to: Option<&str>,
) -> Result<Vec<crate::ledger::BillView>, AppError> {
    // Pull through load_reports path by temporarily using a dedicated query mirroring it.
    // Keep this lean: reuse load_reports for costs/lines by raising limit via raw queries here.
    let rows = sqlx::query_as::<_, BillExportRow>(
        r#"
        SELECT
            b.id,
            b.bill_no,
            s.name AS section_name,
            s.short_name,
            b.total,
            to_char(b.created_at AT TIME ZONE 'Asia/Karachi', 'YYYY-MM-DD HH24:MI') AS created_at,
            u.name AS cashier_name,
            b.customer,
            COALESCE((
                SELECT SUM(
                    CASE
                        WHEN bl.cost_total <> 0 THEN bl.cost_total
                        ELSE i.avg_cost * bl.qty::bigint * bl.units::bigint
                    END
                )
                FROM bill_lines bl
                JOIN items i ON i.id = bl.item_id
                WHERE bl.bill_id = b.id
            ), 0)::bigint AS cost,
            COALESCE((
                SELECT string_agg(bl.name || ' ×' || bl.qty::text, ', ' ORDER BY bl.id)
                FROM bill_lines bl
                WHERE bl.bill_id = b.id
            ), '') AS summary
        FROM bills b
        JOIN sections s ON s.id = b.section_id
        LEFT JOIN users u ON u.id = b.user_id
        WHERE ($1::text IS NULL OR b.created_at >= $1::timestamptz)
          AND ($2::text IS NULL OR b.created_at < $2::timestamptz)
        ORDER BY b.created_at DESC, b.id DESC
        LIMIT $3
        "#,
    )
    .bind(from)
    .bind(to)
    .bind(EXPORT_LIMIT)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| crate::ledger::BillView {
            id: row.id,
            bill_no: row.bill_no,
            section_name: row.section_name,
            short_name: row.short_name,
            accent: String::new(),
            total: row.total,
            cost: row.cost,
            created_at: row.created_at,
            cashier_name: row.cashier_name,
            customer: row.customer,
            lines: vec![crate::ledger::BillLineView {
                name: row.summary,
                qty: 1,
                amount: row.total,
            }],
        })
        .collect())
}

#[derive(sqlx::FromRow)]
struct BillExportRow {
    id: i32,
    bill_no: String,
    section_name: String,
    short_name: String,
    total: i32,
    created_at: String,
    cashier_name: Option<String>,
    customer: Option<String>,
    cost: i64,
    summary: String,
}

async fn load_export_movements(
    pool: &PgPool,
    from: Option<&str>,
    to: Option<&str>,
) -> Result<Vec<StockMovement>, AppError> {
    sqlx::query_as::<_, StockMovement>(
        r#"
        SELECT
            m.id,
            m.item_id,
            i.name AS item_name,
            s.short_name AS section_name,
            m.qty,
            m.reason,
            b.bill_no,
            to_char(m.created_at AT TIME ZONE 'Asia/Karachi', 'YYYY-MM-DD HH24:MI') AS created_at
        FROM stock_movements m
        JOIN items i ON i.id = m.item_id
        JOIN sections s ON s.id = i.section_id
        LEFT JOIN bills b ON b.id = m.bill_id
        WHERE ($1::text IS NULL OR m.created_at >= $1::timestamptz)
          AND ($2::text IS NULL OR m.created_at < $2::timestamptz)
        ORDER BY m.created_at DESC, m.id DESC
        LIMIT $3
        "#,
    )
    .bind(from)
    .bind(to)
    .bind(EXPORT_LIMIT)
    .fetch_all(pool)
    .await
    .map_err(AppError::from)
}

async fn product_mix_export(
    pool: &PgPool,
    from: Option<&str>,
    to: Option<&str>,
) -> Result<crate::ledger::ProductMix, AppError> {
    let mut mix = product_mix(pool, from, to).await?;
    // product_mix caps at 400; reload without that cap for exports.
    let lines = sqlx::query_as::<_, MixLine>(
        r#"
        SELECT
            i.id,
            i.barcode,
            i.name,
            s.short_name AS section_name,
            COALESCE(SUM(CASE WHEN m.qty > 0 THEN m.qty ELSE 0 END), 0)::bigint AS qty_in,
            COALESCE(SUM(CASE WHEN m.qty < 0 THEN -m.qty ELSE 0 END), 0)::bigint AS qty_out,
            (
                i.stock::bigint - COALESCE((
                    SELECT SUM(earlier.qty)::bigint
                    FROM stock_movements earlier
                    WHERE earlier.item_id = i.id
                        AND ($1::text IS NULL OR earlier.created_at >= $1::timestamptz)
                ), 0)
            ) AS opening
        FROM items i
        JOIN sections s ON s.id = i.section_id
        JOIN stock_movements m ON m.item_id = i.id
            AND ($1::text IS NULL OR m.created_at >= $1::timestamptz)
            AND ($2::text IS NULL OR m.created_at < $2::timestamptz)
        WHERE i.active
        GROUP BY i.id, i.barcode, i.name, s.short_name, s.pos_order
        ORDER BY s.pos_order, i.name
        LIMIT $3
        "#,
    )
    .bind(from)
    .bind(to)
    .bind(EXPORT_LIMIT)
    .fetch_all(pool)
    .await?;
    mix.truncated = false;
    mix.lines = lines;
    Ok(mix)
}

fn money_major(amount: i64) -> String {
    amount.to_string()
}

fn money_cost(cents: i64) -> String {
    let sign = if cents < 0 { "-" } else { "" };
    let abs = cents.unsigned_abs();
    let major = abs / 100;
    let frac = abs % 100;
    if frac == 0 {
        format!("{sign}{major}")
    } else {
        format!("{sign}{major}.{:02}", frac)
    }
}

fn line_summary(bill: &crate::ledger::BillView) -> String {
    bill.lines
        .first()
        .map(|line| line.name.clone())
        .unwrap_or_default()
}

async fn export_sales_xlsx(
    pool: &PgPool,
    from: Option<&str>,
    to: Option<&str>,
) -> Result<Vec<u8>, AppError> {
    let report = load_full_reports(pool, from, to).await?;
    let mut workbook = Workbook::new();
    let header = Format::new().set_bold().set_background_color(Color::RGB(0xE2E8F0));
    let sheet = workbook.add_worksheet();
    sheet.set_name("Sales").sheet()?;
    sheet
        .write_string(0, 0, &format!("Sales report — {}", report.range_label))
        .sheet()?;
    sheet
        .write_string(
            1,
            0,
            &format!(
                "Total {} · Profit {} · Bills {}",
                money_major(report.total),
                money_cost(report.profit),
                report.bill_count
            ),
        )
        .sheet()?;

    let headers = [
        "Bill #",
        "When",
        "Head",
        "Cashier",
        "Customer",
        "Items",
        "Total",
        "Profit",
    ];
    for (col, name) in headers.iter().enumerate() {
        sheet
            .write_string_with_format(3, col as u16, *name, &header)
            .sheet()?;
    }
    for (index, bill) in report.bills.iter().enumerate() {
        let row = (index + 4) as u32;
        let profit = i64::from(bill.total) * 100 - bill.cost;
        sheet.write_string(row, 0, &bill.bill_no).sheet()?;
        sheet.write_string(row, 1, &bill.created_at).sheet()?;
        sheet.write_string(row, 2, &bill.short_name).sheet()?;
        sheet
            .write_string(row, 3, bill.cashier_name.as_deref().unwrap_or(""))
            .sheet()?;
        sheet
            .write_string(row, 4, bill.customer.as_deref().unwrap_or(""))
            .sheet()?;
        sheet.write_string(row, 5, &line_summary(bill)).sheet()?;
        sheet
            .write_number(row, 6, bill.total as f64)
            .sheet()?;
        sheet
            .write_string(row, 7, &money_cost(profit))
            .sheet()?;
    }
    sheet.set_freeze_panes(4, 0).sheet()?;
    set_widths(sheet, &[14.0, 18.0, 12.0, 16.0, 18.0, 40.0, 12.0, 12.0])?;
    workbook.save_to_buffer().sheet()
}

async fn export_ledger_xlsx(
    pool: &PgPool,
    from: Option<&str>,
    to: Option<&str>,
) -> Result<Vec<u8>, AppError> {
    let report = load_full_reports(pool, from, to).await?;
    let mut workbook = Workbook::new();
    let header = Format::new().set_bold().set_background_color(Color::RGB(0xE2E8F0));
    let sheet = workbook.add_worksheet();
    sheet.set_name("Ledger").sheet()?;
    sheet
        .write_string(0, 0, &format!("Stock ledger — {}", report.range_label))
        .sheet()?;
    sheet
        .write_string(
            1,
            0,
            &format!(
                "On hand {} · Valuation {}",
                report.stock_units,
                money_cost(report.stock_value)
            ),
        )
        .sheet()?;
    let headers = ["When", "Item", "Head", "Qty", "Direction", "Note", "Bill"];
    for (col, name) in headers.iter().enumerate() {
        sheet
            .write_string_with_format(3, col as u16, *name, &header)
            .sheet()?;
    }
    for (index, move_row) in report.movements.iter().enumerate() {
        let row = (index + 4) as u32;
        sheet.write_string(row, 0, &move_row.created_at).sheet()?;
        sheet.write_string(row, 1, &move_row.item_name).sheet()?;
        sheet.write_string(row, 2, &move_row.section_name).sheet()?;
        sheet
            .write_number(row, 3, move_row.qty.abs() as f64)
            .sheet()?;
        sheet
            .write_string(row, 4, if move_row.qty < 0 { "out" } else { "in" })
            .sheet()?;
        sheet.write_string(row, 5, &move_row.reason).sheet()?;
        write_text(
            sheet,
            row,
            6,
            move_row.bill_no.as_deref().unwrap_or(""),
            &Format::new().set_num_format("@"),
        )?;
    }
    sheet.set_freeze_panes(4, 0).sheet()?;
    set_widths(sheet, &[18.0, 28.0, 12.0, 10.0, 10.0, 36.0, 14.0])?;
    workbook.save_to_buffer().sheet()
}

async fn export_mix_xlsx(
    pool: &PgPool,
    from: Option<&str>,
    to: Option<&str>,
) -> Result<Vec<u8>, AppError> {
    let mix = product_mix_export(pool, from, to).await?;
    let mut workbook = Workbook::new();
    let header = Format::new().set_bold().set_background_color(Color::RGB(0xE2E8F0));
    let sheet = workbook.add_worksheet();
    sheet.set_name("Product mix").sheet()?;
    sheet
        .write_string(0, 0, &format!("Product mix — {}", mix.label))
        .sheet()?;
    let headers = ["Item", "Barcode", "Head", "Opening", "In", "Out", "Net"];
    for (col, name) in headers.iter().enumerate() {
        sheet
            .write_string_with_format(2, col as u16, *name, &header)
            .sheet()?;
    }
    for (index, line) in mix.lines.iter().enumerate() {
        let row = (index + 3) as u32;
        let net = line.opening + line.qty_in - line.qty_out;
        sheet.write_string(row, 0, &line.name).sheet()?;
        write_text(
            sheet,
            row,
            1,
            line.barcode.as_deref().unwrap_or(""),
            &Format::new().set_num_format("@"),
        )?;
        sheet.write_string(row, 2, &line.section_name).sheet()?;
        sheet.write_number(row, 3, line.opening as f64).sheet()?;
        sheet.write_number(row, 4, line.qty_in as f64).sheet()?;
        sheet.write_number(row, 5, line.qty_out as f64).sheet()?;
        sheet.write_number(row, 6, net as f64).sheet()?;
    }
    sheet.set_freeze_panes(3, 0).sheet()?;
    set_widths(sheet, &[28.0, 16.0, 12.0, 12.0, 10.0, 10.0, 10.0])?;
    workbook.save_to_buffer().sheet()
}

async fn export_sales_pdf(
    pool: &PgPool,
    from: Option<&str>,
    to: Option<&str>,
) -> Result<Vec<u8>, AppError> {
    let report = load_full_reports(pool, from, to).await?;
    let headers = ["Bill #", "When", "Head", "Items", "Total", "Profit"];
    // Letter landscape: 792 x 612
    let widths = [90.0_f32, 110.0, 70.0, 280.0, 70.0, 70.0];
    let rows: Vec<Vec<String>> = report
        .bills
        .iter()
        .map(|bill| {
            let profit = i64::from(bill.total) * 100 - bill.cost;
            vec![
                bill.bill_no.clone(),
                bill.created_at.clone(),
                bill.short_name.clone(),
                line_summary(bill),
                bill.total.to_string(),
                money_cost(profit),
            ]
        })
        .collect();
    Ok(table_pdf(
        &format!("Sales report — {}", report.range_label),
        &format!(
            "Total {} · Profit {} · Bills {}",
            money_major(report.total),
            money_cost(report.profit),
            report.bill_count
        ),
        &headers,
        &widths,
        &rows,
        &[4, 5],
    ))
}

async fn export_ledger_pdf(
    pool: &PgPool,
    from: Option<&str>,
    to: Option<&str>,
) -> Result<Vec<u8>, AppError> {
    let report = load_full_reports(pool, from, to).await?;
    let headers = ["When", "Item", "Head", "Qty", "Note", "Bill"];
    let widths = [110.0_f32, 180.0, 70.0, 50.0, 220.0, 80.0];
    let rows: Vec<Vec<String>> = report
        .movements
        .iter()
        .map(|move_row| {
            vec![
                move_row.created_at.clone(),
                move_row.item_name.clone(),
                move_row.section_name.clone(),
                if move_row.qty < 0 {
                    move_row.qty.to_string()
                } else {
                    format!("+{}", move_row.qty)
                },
                move_row.reason.clone(),
                move_row.bill_no.clone().unwrap_or_default(),
            ]
        })
        .collect();
    Ok(table_pdf(
        &format!("Stock ledger — {}", report.range_label),
        &format!(
            "On hand {} · Valuation {}",
            report.stock_units,
            money_cost(report.stock_value)
        ),
        &headers,
        &widths,
        &rows,
        &[3],
    ))
}

async fn export_mix_pdf(
    pool: &PgPool,
    from: Option<&str>,
    to: Option<&str>,
) -> Result<Vec<u8>, AppError> {
    let mix = product_mix_export(pool, from, to).await?;
    let headers = ["Item", "Head", "Opening", "In", "Out", "Net"];
    let widths = [260.0_f32, 90.0, 70.0, 70.0, 70.0, 70.0];
    let rows: Vec<Vec<String>> = mix
        .lines
        .iter()
        .map(|line| {
            let net = line.opening + line.qty_in - line.qty_out;
            vec![
                line.name.clone(),
                line.section_name.clone(),
                line.opening.to_string(),
                format!("+{}", line.qty_in),
                format!("-{}", line.qty_out),
                net.to_string(),
            ]
        })
        .collect();
    Ok(table_pdf(
        &format!("Product mix — {}", mix.label),
        "",
        &headers,
        &widths,
        &rows,
        &[2, 3, 4, 5],
    ))
}

/// Landscape letter PDF with a ruled table (Helvetica).
fn table_pdf(
    title: &str,
    subtitle: &str,
    headers: &[&str],
    widths: &[f32],
    rows: &[Vec<String>],
    right_cols: &[usize],
) -> Vec<u8> {
    let (page_w, page_h) = (792.0_f32, 612.0);
    let margin = 36.0_f32;
    let table_w: f32 = widths.iter().sum();
    let left = margin;
    let top = page_h - margin;
    let row_h = 18.0_f32;
    let header_h = 20.0_f32;
    let title_block = if subtitle.is_empty() { 36.0 } else { 52.0 };
    let usable = top - margin - title_block - header_h;
    let rows_per_page = ((usable / row_h).floor() as usize).max(1);

    let chunks: Vec<&[Vec<String>]> = if rows.is_empty() {
        vec![&[]]
    } else {
        rows.chunks(rows_per_page).collect()
    };

    let mut content_objects = Vec::new();
    for (page_index, chunk) in chunks.iter().enumerate() {
        let mut stream = String::new();
        // Title
        stream.push_str(&format!(
            "BT /F1 14 Tf {} {} Td ({}) Tj ET\n",
            left,
            top - 14.0,
            pdf_escape(title)
        ));
        if !subtitle.is_empty() {
            stream.push_str(&format!(
                "BT /F1 9 Tf {} {} Td ({}) Tj ET\n",
                left,
                top - 30.0,
                pdf_escape(subtitle)
            ));
        }
        if chunks.len() > 1 {
            stream.push_str(&format!(
                "BT /F1 9 Tf {} {} Td ({}) Tj ET\n",
                page_w - margin - 70.0,
                top - 14.0,
                pdf_escape(&format!("Page {}/{}", page_index + 1, chunks.len()))
            ));
        }

        let table_top = top - title_block;
        let body_h = chunk.len() as f32 * row_h;
        let table_h = header_h + body_h;

        // Outer border
        stream.push_str("0.6 w 0.15 0.2 0.3 RG 0.95 0.96 0.98 rg\n");
        stream.push_str(&format!(
            "{} {} {} {} re B\n",
            left,
            table_top - table_h,
            table_w,
            table_h
        ));

        // Header fill
        stream.push_str("0.89 0.91 0.94 rg\n");
        stream.push_str(&format!(
            "{} {} {} {} re f\n",
            left,
            table_top - header_h,
            table_w,
            header_h
        ));

        // Vertical grid
        stream.push_str("0.75 0.78 0.82 RG 0.5 w\n");
        let mut x = left;
        for width in widths.iter().take(widths.len().saturating_sub(1)) {
            x += width;
            stream.push_str(&format!(
                "{} {} m {} {} l S\n",
                x,
                table_top,
                x,
                table_top - table_h
            ));
        }
        // Header bottom rule
        stream.push_str(&format!(
            "{} {} m {} {} l S\n",
            left,
            table_top - header_h,
            left + table_w,
            table_top - header_h
        ));
        // Row rules
        for i in 1..=chunk.len() {
            let y = table_top - header_h - i as f32 * row_h;
            stream.push_str(&format!(
                "{} {} m {} {} l S\n",
                left,
                y,
                left + table_w,
                y
            ));
        }

        // Header labels
        let mut hx = left;
        for (col, header) in headers.iter().enumerate() {
            let pad = 4.0;
            stream.push_str(&format!(
                "BT /F2 8 Tf {} {} Td ({}) Tj ET\n",
                hx + pad,
                table_top - header_h + 6.0,
                pdf_escape(header)
            ));
            hx += widths[col];
        }

        // Body cells
        for (row_index, row) in chunk.iter().enumerate() {
            let y = table_top - header_h - (row_index as f32 + 1.0) * row_h + 5.0;
            let mut cx = left;
            for (col, cell) in row.iter().enumerate() {
                let width = widths.get(col).copied().unwrap_or(60.0);
                let max_chars = ((width - 8.0) / 4.6).floor().max(4.0) as usize;
                let text = clip(cell, max_chars);
                let right_align = right_cols.contains(&col);
                let tx = if right_align {
                    cx + width - 4.0 - approx_text_width(&text)
                } else {
                    cx + 4.0
                };
                stream.push_str(&format!(
                    "BT /F1 8 Tf {} {} Td ({}) Tj ET\n",
                    tx.max(cx + 2.0),
                    y,
                    pdf_escape(&text)
                ));
                cx += width;
            }
        }

        content_objects.push(stream);
    }

    assemble_pdf(&content_objects, page_w, page_h)
}

fn approx_text_width(text: &str) -> f32 {
    text.chars().count() as f32 * 4.6
}

fn clip(value: &str, width: usize) -> String {
    let trimmed = value.trim();
    if trimmed.chars().count() <= width {
        return trimmed.to_string();
    }
    let mut out: String = trimmed.chars().take(width.saturating_sub(1)).collect();
    out.push('…');
    out
}

fn assemble_pdf(content_objects: &[String], page_w: f32, page_h: f32) -> Vec<u8> {
    let mut pdf = String::from("%PDF-1.4\n");
    let mut offsets: Vec<usize> = Vec::new();
    let write_obj = |pdf: &mut String, offsets: &mut Vec<usize>, body: &str| {
        offsets.push(pdf.len());
        let id = offsets.len();
        pdf.push_str(&format!("{id} 0 obj\n{body}\nendobj\n"));
        id
    };

    // 1 catalog, 2 pages, 3 Helvetica, 4 Helvetica-Bold, then page/content pairs from 5.
    write_obj(&mut pdf, &mut offsets, "<< /Type /Catalog /Pages 2 0 R >>");
    let page_count = content_objects.len().max(1);
    let first_page_id = 5;
    let kids: String = (0..page_count)
        .map(|index| format!("{} 0 R", first_page_id + index * 2))
        .collect::<Vec<_>>()
        .join(" ");
    write_obj(
        &mut pdf,
        &mut offsets,
        &format!("<< /Type /Pages /Kids [{kids}] /Count {page_count} >>"),
    );
    write_obj(
        &mut pdf,
        &mut offsets,
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>",
    );
    write_obj(
        &mut pdf,
        &mut offsets,
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica-Bold >>",
    );

    let pages = if content_objects.is_empty() {
        vec!["BT /F1 12 Tf 72 540 Td (No rows) Tj ET".to_string()]
    } else {
        content_objects.to_vec()
    };

    for content in &pages {
        let content_id = offsets.len() + 2;
        write_obj(
            &mut pdf,
            &mut offsets,
            &format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {} {}] /Resources << /Font << /F1 3 0 R /F2 4 0 R >> >> /Contents {content_id} 0 R >>",
                page_w as i32, page_h as i32
            ),
        );
        write_obj(
            &mut pdf,
            &mut offsets,
            &format!(
                "<< /Length {} >>\nstream\n{}\nendstream",
                content.len(),
                content
            ),
        );
    }

    let xref_at = pdf.len();
    pdf.push_str(&format!("xref\n0 {}\n", offsets.len() + 1));
    pdf.push_str("0000000000 65535 f \n");
    for offset in &offsets {
        pdf.push_str(&format!("{offset:010} 00000 n \n"));
    }
    pdf.push_str(&format!(
        "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref_at}\n%%EOF\n",
        offsets.len() + 1
    ));
    pdf.into_bytes()
}

fn pdf_escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '(' => out.push_str("\\("),
            ')' => out.push_str("\\)"),
            '\r' | '\n' | '\t' => out.push(' '),
            c if c.is_ascii() && !c.is_control() => out.push(c),
            _ => out.push('?'),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pdf_draws_ruled_table_not_pipe_text() {
        let rows = vec![
            vec![
                "BILL-1001".into(),
                "2026-11-27 10:00".into(),
                "WH".into(),
                "Pan x1".into(),
                "250".into(),
                "40.00".into(),
            ];
            3
        ];
        let pdf = table_pdf(
            "Sales report — Today",
            "Total 750 · Profit 120.00 · Bills 3",
            &["Bill #", "When", "Head", "Items", "Total", "Profit"],
            &[90.0, 110.0, 70.0, 280.0, 70.0, 70.0],
            &rows,
            &[4, 5],
        );
        let text = String::from_utf8_lossy(&pdf);
        assert!(text.starts_with("%PDF-1.4"));
        assert!(text.contains("/MediaBox [0 0 792 612]"));
        assert!(text.contains(" re "));
        assert!(text.contains(" m ") && text.contains(" l "));
        assert!(!text.contains(" | "));
        assert!(text.contains("(Bill #)"));
        assert!(text.contains("(BILL-1001)"));
    }
}
