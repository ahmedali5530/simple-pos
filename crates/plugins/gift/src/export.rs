use axum::http::{header, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use platform_kernel::error::AppError;
use platform_sheets::{set_widths, table_pdf, SheetResult};
use rust_xlsxwriter::{Color, Format, Workbook};

use crate::routes::LiabilityRow;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    Xlsx,
    Pdf,
}

impl ExportFormat {
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

pub fn build(
    format: ExportFormat,
    cards: i64,
    issued: i64,
    outstanding: i64,
    rows: &[LiabilityRow],
) -> Result<Vec<u8>, AppError> {
    match format {
        ExportFormat::Xlsx => xlsx(cards, issued, outstanding, rows),
        ExportFormat::Pdf => Ok(pdf(cards, issued, outstanding, rows)),
    }
}

fn xlsx(
    cards: i64,
    issued: i64,
    outstanding: i64,
    rows: &[LiabilityRow],
) -> Result<Vec<u8>, AppError> {
    let mut workbook = Workbook::new();
    let header = Format::new()
        .set_bold()
        .set_background_color(Color::RGB(0xE2E8F0));
    let sheet = workbook.add_worksheet();
    sheet.set_name("Gift liability").sheet()?;
    sheet.write_string(0, 0, "Gift liability").sheet()?;
    sheet
        .write_string(
            1,
            0,
            &format!("Cards {cards} · Issued {issued} · Outstanding {outstanding}"),
        )
        .sheet()?;
    for (col, name) in ["Code", "Issued", "Balance", "Status", "Bill", "When"]
        .iter()
        .enumerate()
    {
        sheet
            .write_string_with_format(3, col as u16, *name, &header)
            .sheet()?;
    }
    for (index, row) in rows.iter().enumerate() {
        let r = (index + 4) as u32;
        sheet.write_string(r, 0, &row.code).sheet()?;
        sheet.write_number(r, 1, row.issued as f64).sheet()?;
        sheet.write_number(r, 2, row.balance as f64).sheet()?;
        sheet.write_string(r, 3, &row.status).sheet()?;
        sheet.write_string(r, 4, &row.issued_bill_no).sheet()?;
        sheet
            .write_string(r, 5, row.issued_at.as_deref().unwrap_or(""))
            .sheet()?;
    }
    sheet.set_freeze_panes(4, 0).sheet()?;
    set_widths(sheet, &[16.0, 12.0, 12.0, 10.0, 14.0, 22.0])?;
    workbook.save_to_buffer().sheet()
}

fn pdf(cards: i64, issued: i64, outstanding: i64, rows: &[LiabilityRow]) -> Vec<u8> {
    let table_rows: Vec<Vec<String>> = rows
        .iter()
        .map(|row| {
            vec![
                row.code.clone(),
                row.issued.to_string(),
                row.balance.to_string(),
                row.status.clone(),
                row.issued_bill_no.clone(),
                row.issued_at.clone().unwrap_or_default(),
            ]
        })
        .collect();
    table_pdf(
        "Gift liability",
        &format!("Cards {cards} · Issued {issued} · Outstanding {outstanding}"),
        &["Code", "Issued", "Balance", "Status", "Bill", "When"],
        &[120.0, 80.0, 80.0, 70.0, 100.0, 160.0],
        &table_rows,
        &[1, 2],
    )
}
