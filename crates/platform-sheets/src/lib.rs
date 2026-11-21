//! Neutral Excel helpers shared by catalog and inventory plugins.

use std::collections::HashMap;
use std::io::Cursor;

use calamine::{Data, Reader, Xlsx};
use platform_kernel::error::AppError;
use rust_xlsxwriter::Workbook;

pub fn open_book(bytes: &[u8]) -> Result<Xlsx<Cursor<&[u8]>>, AppError> {
    Xlsx::new(Cursor::new(bytes))
        .map_err(|_| AppError::BadRequest("That workbook could not be read.".into()))
}

pub fn header_map(row: &[Data]) -> HashMap<String, usize> {
    row.iter()
        .enumerate()
        .filter_map(|(index, cell)| {
            let text = cell_string(cell).trim().to_ascii_lowercase();
            if text.is_empty() {
                None
            } else {
                Some((text, index))
            }
        })
        .collect()
}

pub fn cell_string(cell: &Data) -> String {
    match cell {
        Data::Empty => String::new(),
        Data::String(s) => s.clone(),
        Data::Float(f) => {
            if f.fract() == 0.0 {
                format!("{}", *f as i64)
            } else {
                f.to_string()
            }
        }
        Data::Int(i) => i.to_string(),
        Data::Bool(b) => b.to_string(),
        Data::DateTime(dt) => dt.to_string(),
        Data::DateTimeIso(s) | Data::DurationIso(s) => s.clone(),
        Data::Error(e) => e.to_string(),
    }
}

pub fn new_workbook() -> Workbook {
    Workbook::new()
}

pub fn sheet_error(message: impl Into<String>) -> AppError {
    AppError::BadRequest(message.into())
}
