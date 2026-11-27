use axum::http::{header, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use platform_kernel::error::AppError;
use platform_sheets::{set_widths, table_pdf, SheetResult};
use rust_xlsxwriter::{Color, Format, Workbook};

use crate::submit::FiscalSubmission;

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

pub fn build(format: ExportFormat, rows: &[FiscalSubmission]) -> Result<Vec<u8>, AppError> {
    match format {
        ExportFormat::Xlsx => xlsx(rows),
        ExportFormat::Pdf => Ok(pdf(rows)),
    }
}

fn xlsx(rows: &[FiscalSubmission]) -> Result<Vec<u8>, AppError> {
    let mut workbook = Workbook::new();
    let header = Format::new()
        .set_bold()
        .set_background_color(Color::RGB(0xE2E8F0));
    let sheet = workbook.add_worksheet();
    sheet.set_name("Fiscal").sheet()?;
    sheet
        .write_string(0, 0, "Fiscal submissions")
        .sheet()?;
    for (col, name) in ["Bill #", "Invoice", "Authority", "Status", "When", "Error"]
        .iter()
        .enumerate()
    {
        sheet
            .write_string_with_format(2, col as u16, *name, &header)
            .sheet()?;
    }
    for (index, row) in rows.iter().enumerate() {
        let r = (index + 3) as u32;
        sheet.write_string(r, 0, &row.bill_no).sheet()?;
        sheet.write_string(r, 1, &row.invoice_no).sheet()?;
        sheet.write_string(r, 2, &row.authority).sheet()?;
        sheet.write_string(r, 3, &row.status).sheet()?;
        sheet.write_string(r, 4, &row.submitted_at).sheet()?;
        sheet.write_string(r, 5, &row.error_message).sheet()?;
    }
    sheet.set_freeze_panes(3, 0).sheet()?;
    set_widths(sheet, &[14.0, 18.0, 14.0, 10.0, 22.0, 36.0])?;
    workbook.save_to_buffer().sheet()
}

fn pdf(rows: &[FiscalSubmission]) -> Vec<u8> {
    let table_rows: Vec<Vec<String>> = rows
        .iter()
        .map(|row| {
            vec![
                row.bill_no.clone(),
                row.invoice_no.clone(),
                row.authority.clone(),
                row.status.clone(),
                row.submitted_at.clone(),
                row.error_message.clone(),
            ]
        })
        .collect();
    table_pdf(
        "Fiscal submissions",
        &format!("{} rows", rows.len()),
        &["Bill #", "Invoice", "Authority", "Status", "When", "Error"],
        &[90.0, 110.0, 90.0, 70.0, 130.0, 220.0],
        &table_rows,
        &[],
    )
}
