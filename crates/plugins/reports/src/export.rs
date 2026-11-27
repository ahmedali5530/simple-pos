//! Downloadable sales / ledger / mix reports for a chosen date range.

use axum::http::{header, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use inventory_contract::InventoryReportsProvider;
use platform_kernel::error::AppError;
use platform_kernel::routes::TenantDb;
use platform_sheets::{set_widths, table_pdf, write_text, SheetResult};
use rust_xlsxwriter::{Color, Format, Workbook};
use sales_contract::SalesReportingProvider;

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

pub fn file_response(filename: &str, content_type: &'static str, bytes: Vec<u8>) -> Response {
    let disposition = HeaderValue::from_str(&format!("attachment; filename=\"{filename}\""))
        .unwrap_or_else(|_| HeaderValue::from_static("attachment; filename=\"report.bin\""));
    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, HeaderValue::from_static(content_type)),
            (header::CONTENT_DISPOSITION, disposition),
        ],
        bytes,
    )
        .into_response()
}

pub async fn export_report(
    db: &TenantDb,
    kind: ReportKind,
    format: ReportFormat,
    from: Option<&str>,
    to: Option<&str>,
) -> Result<Vec<u8>, AppError> {
    match (kind, format) {
        (ReportKind::Sales, ReportFormat::Xlsx) => export_sales_xlsx(db, from, to).await,
        (ReportKind::Sales, ReportFormat::Pdf) => export_sales_pdf(db, from, to).await,
        (ReportKind::Ledger, ReportFormat::Xlsx) => export_ledger_xlsx(db, from, to).await,
        (ReportKind::Ledger, ReportFormat::Pdf) => export_ledger_pdf(db, from, to).await,
        (ReportKind::Mix, ReportFormat::Xlsx) => export_mix_xlsx(db, from, to).await,
        (ReportKind::Mix, ReportFormat::Pdf) => export_mix_pdf(db, from, to).await,
    }
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

fn line_summary(bill: &sales_contract::SalesBill) -> String {
    bill.lines
        .iter()
        .map(|line| format!("{} ×{}", line.name, line.qty))
        .collect::<Vec<_>>()
        .join(", ")
}

fn clock_label(value: &str) -> String {
    // Keep ISO timestamps readable in exports when already local-formatted.
    if let Some((date, rest)) = value.split_once('T') {
        let time = rest.trim_end_matches('Z');
        if time.len() >= 5 {
            return format!("{date} {}", &time[..5]);
        }
    }
    value.to_string()
}

async fn load_sales(
    db: &TenantDb,
    from: Option<&str>,
    to: Option<&str>,
) -> Result<sales_contract::SalesReport, AppError> {
    let sales = db
        .provider::<SalesReportingProvider>()
        .ok_or_else(|| AppError::Conflict("Point of Sale is not enabled.".into()))?;
    let mut tx = db.begin(true).await?;
    let report = sales.0.report(&mut tx, from, to).await?;
    tx.commit().await?;
    Ok(report)
}

async fn load_stock(
    db: &TenantDb,
    from: Option<&str>,
    to: Option<&str>,
) -> Result<inventory_contract::StockReport, AppError> {
    let stock = db
        .provider::<InventoryReportsProvider>()
        .ok_or_else(|| AppError::Conflict("Inventory is not enabled.".into()))?;
    let mut tx = db.begin(true).await?;
    let report = stock.0.stock_report(&mut tx, from, to).await?;
    tx.commit().await?;
    Ok(report)
}

async fn export_sales_xlsx(
    db: &TenantDb,
    from: Option<&str>,
    to: Option<&str>,
) -> Result<Vec<u8>, AppError> {
    let report = load_sales(db, from, to).await?;
    let mut workbook = Workbook::new();
    let header = Format::new()
        .set_bold()
        .set_background_color(Color::RGB(0xE2E8F0));
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
        sheet
            .write_string(row, 1, &clock_label(&bill.created_at))
            .sheet()?;
        sheet.write_string(row, 2, &bill.short_name).sheet()?;
        sheet
            .write_string(row, 3, bill.cashier_name.as_deref().unwrap_or(""))
            .sheet()?;
        sheet
            .write_string(row, 4, bill.customer.as_deref().unwrap_or(""))
            .sheet()?;
        sheet.write_string(row, 5, &line_summary(bill)).sheet()?;
        sheet.write_number(row, 6, bill.total as f64).sheet()?;
        sheet.write_string(row, 7, &money_cost(profit)).sheet()?;
    }
    sheet.set_freeze_panes(4, 0).sheet()?;
    set_widths(sheet, &[14.0, 18.0, 12.0, 16.0, 18.0, 40.0, 12.0, 12.0])?;
    workbook.save_to_buffer().sheet()
}

async fn export_ledger_xlsx(
    db: &TenantDb,
    from: Option<&str>,
    to: Option<&str>,
) -> Result<Vec<u8>, AppError> {
    let report = load_stock(db, from, to).await?;
    let mut workbook = Workbook::new();
    let header = Format::new()
        .set_bold()
        .set_background_color(Color::RGB(0xE2E8F0));
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
        sheet
            .write_string(row, 0, &clock_label(&move_row.created_at))
            .sheet()?;
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
    db: &TenantDb,
    from: Option<&str>,
    to: Option<&str>,
) -> Result<Vec<u8>, AppError> {
    let report = load_stock(db, from, to).await?;
    let mut workbook = Workbook::new();
    let header = Format::new()
        .set_bold()
        .set_background_color(Color::RGB(0xE2E8F0));
    let sheet = workbook.add_worksheet();
    sheet.set_name("Product mix").sheet()?;
    sheet
        .write_string(0, 0, &format!("Product mix — {}", report.range_label))
        .sheet()?;
    let headers = ["Item", "Barcode", "Head", "Opening", "In", "Out", "Net"];
    for (col, name) in headers.iter().enumerate() {
        sheet
            .write_string_with_format(2, col as u16, *name, &header)
            .sheet()?;
    }
    for (index, line) in report.mix.iter().enumerate() {
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
    db: &TenantDb,
    from: Option<&str>,
    to: Option<&str>,
) -> Result<Vec<u8>, AppError> {
    let report = load_sales(db, from, to).await?;
    let headers = ["Bill #", "When", "Head", "Items", "Total", "Profit"];
    let widths = [90.0_f32, 110.0, 70.0, 280.0, 70.0, 70.0];
    let rows: Vec<Vec<String>> = report
        .bills
        .iter()
        .map(|bill| {
            let profit = i64::from(bill.total) * 100 - bill.cost;
            vec![
                bill.bill_no.clone(),
                clock_label(&bill.created_at),
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
    db: &TenantDb,
    from: Option<&str>,
    to: Option<&str>,
) -> Result<Vec<u8>, AppError> {
    let report = load_stock(db, from, to).await?;
    let headers = ["When", "Item", "Head", "Qty", "Note", "Bill"];
    let widths = [110.0_f32, 180.0, 70.0, 50.0, 220.0, 80.0];
    let rows: Vec<Vec<String>> = report
        .movements
        .iter()
        .map(|move_row| {
            vec![
                clock_label(&move_row.created_at),
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
    db: &TenantDb,
    from: Option<&str>,
    to: Option<&str>,
) -> Result<Vec<u8>, AppError> {
    let report = load_stock(db, from, to).await?;
    let headers = ["Item", "Head", "Opening", "In", "Out", "Net"];
    let widths = [260.0_f32, 90.0, 70.0, 70.0, 70.0, 70.0];
    let rows: Vec<Vec<String>> = report
        .mix
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
        &format!("Product mix — {}", report.range_label),
        "",
        &headers,
        &widths,
        &rows,
        &[2, 3, 4, 5],
    ))
}
