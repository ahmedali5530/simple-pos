use axum::http::{header, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use platform_kernel::error::AppError;
use platform_sheets::{set_widths, table_pdf, SheetResult};
use rust_xlsxwriter::{Color, Format, Workbook};

use crate::routes::MethodTotal;

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
    totals: &[MethodTotal],
    collected: i64,
    range_label: &str,
) -> Result<Vec<u8>, AppError> {
    match format {
        ExportFormat::Xlsx => xlsx(totals, collected, range_label),
        ExportFormat::Pdf => Ok(pdf(totals, collected, range_label)),
    }
}

fn xlsx(totals: &[MethodTotal], collected: i64, range_label: &str) -> Result<Vec<u8>, AppError> {
    let mut workbook = Workbook::new();
    let header = Format::new()
        .set_bold()
        .set_background_color(Color::RGB(0xE2E8F0));
    let sheet = workbook.add_worksheet();
    sheet.set_name("Payments").sheet()?;
    sheet
        .write_string(0, 0, &format!("Payment mix — {range_label}"))
        .sheet()?;
    sheet
        .write_string(1, 0, &format!("Collected {collected}"))
        .sheet()?;
    for (col, name) in ["Method", "Bills", "Amount"].iter().enumerate() {
        sheet
            .write_string_with_format(3, col as u16, *name, &header)
            .sheet()?;
    }
    for (index, row) in totals.iter().enumerate() {
        let r = (index + 4) as u32;
        let label = if row.label.is_empty() {
            row.method.as_str()
        } else {
            row.label.as_str()
        };
        sheet.write_string(r, 0, label).sheet()?;
        sheet.write_number(r, 1, row.bills as f64).sheet()?;
        sheet.write_number(r, 2, row.amount as f64).sheet()?;
    }
    sheet.set_freeze_panes(4, 0).sheet()?;
    set_widths(sheet, &[22.0, 12.0, 14.0])?;
    workbook.save_to_buffer().sheet()
}

fn pdf(totals: &[MethodTotal], collected: i64, range_label: &str) -> Vec<u8> {
    let rows: Vec<Vec<String>> = totals
        .iter()
        .map(|row| {
            let label = if row.label.is_empty() {
                row.method.clone()
            } else {
                row.label.clone()
            };
            vec![label, row.bills.to_string(), row.amount.to_string()]
        })
        .collect();
    table_pdf(
        &format!("Payment mix — {range_label}"),
        &format!("Collected {collected}"),
        &["Method", "Bills", "Amount"],
        &[320.0, 120.0, 140.0],
        &rows,
        &[1, 2],
    )
}
