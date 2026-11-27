use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use cash_contract::CashClosing;
use inventory_contract::ProductMix;
use platform_kernel::context::TenantTx;
use platform_kernel::error::AppError;
use platform_runtime::shop_profile::ShopProfile;
use printing_contract::ReceiptExtras;
use sales_contract::ReceiptData;
use sqlx::{Postgres, Transaction};

const WIDTH: usize = 42;

#[derive(Debug, Deserialize)]
pub struct PrintRequest {
    pub bill_no: String,
}

#[derive(Debug, Serialize)]
pub struct PrintOutcome {
    pub printed: bool,
}

struct ReceiptLine {
    name: String,
    qty: i32,
    rate: i32,
    amount: i32,
}

struct ReceiptTender {
    method: String,
    amount: i32,
    reference: String,
}

struct Receipt {
    shop_name: String,
    currency: String,
    bill_no: String,
    sold_at: String,
    cashier: String,
    customer: Option<String>,
    lines: Vec<ReceiptLine>,
    total: i32,
    tenders: Vec<ReceiptTender>,
}

/// Print a saved bill on the shop printer. A turned-off printer is not an error.
/// `extras` come from other plugins (fiscal QR, loyalty, …) via `ReceiptContribute`.
pub async fn print_saved_bill(
    tx: &mut Transaction<'_, Postgres>,
    shop: &ShopProfile,
    data: ReceiptData,
    extras: &ReceiptExtras,
) -> Result<PrintOutcome, AppError> {
    let Some(printer) = enabled_printer_tx(tx).await? else {
        return Ok(PrintOutcome { printed: false });
    };
    let mut tenders: Vec<ReceiptTender> = data
        .tenders
        .into_iter()
        .map(|tender| ReceiptTender {
            method: tender.method,
            amount: tender.amount,
            reference: tender.reference,
        })
        .collect();
    if tenders.is_empty() {
        tenders.push(ReceiptTender {
            method: "cash".into(),
            amount: data.total,
            reference: String::new(),
        });
    }
    let receipt = Receipt {
        shop_name: shop.name.clone(),
        currency: shop.currency.clone(),
        bill_no: data.bill_no,
        sold_at: data.sold_at,
        cashier: data.cashier,
        customer: data.customer,
        lines: data
            .lines
            .into_iter()
            .map(|line| ReceiptLine {
                name: line.name,
                qty: line.qty,
                rate: line.rate,
                amount: line.amount,
            })
            .collect(),
        total: data.total,
        tenders,
    };
    let bytes = build_final_receipt(&receipt, extras);
    dispatch(&printer, &bytes, "The sale is saved.")?;
    Ok(PrintOutcome { printed: true })
}

pub async fn print_product_mix(
    tx: &mut Transaction<'_, Postgres>,
    shop: &ShopProfile,
    mix: ProductMix,
) -> Result<PrintOutcome, AppError> {
    let Some(printer) = enabled_printer_tx(tx).await? else {
        return Ok(PrintOutcome { printed: false });
    };
    let lines = mix
        .lines
        .iter()
        .map(|line| MixSlipLine {
            name: line.name.clone(),
            qty_in: line.qty_in as i64,
            qty_out: line.qty_out as i64,
        })
        .collect::<Vec<_>>();
    let bytes = build_mix_slip(
        &shop.name,
        &shop.currency,
        &mix.label,
        &lines,
        mix.truncated,
    );
    dispatch(&printer, &bytes, "Nothing else changed.")?;
    Ok(PrintOutcome { printed: true })
}

pub async fn print_cash_shift(
    tx: &mut Transaction<'_, Postgres>,
    shop: &ShopProfile,
    shift: CashClosing,
) -> Result<PrintOutcome, AppError> {
    let Some(printer) = enabled_printer_tx(tx).await? else {
        return Ok(PrintOutcome { printed: false });
    };
    if shift.closed_at.is_none() {
        return Err(AppError::BadRequest(
            "Close the shift before printing it.".into(),
        ));
    }
    let bytes = build_cash_slip(&shop.name, &shop.currency, &shift);
    dispatch(&printer, &bytes, "The closing is saved.")?;
    Ok(PrintOutcome { printed: true })
}

async fn enabled_printer_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
) -> Result<Option<PrinterRow>, AppError> {
    let printer = sqlx::query_as::<_, PrinterRow>(
        r#"
        SELECT enabled AS print_enabled, kind AS printer_kind, target AS printer_target, port AS printer_port
        FROM printing_settings
        WHERE tenant_id = current_setting('app.tenant_id')::uuid
        "#,
    )
    .fetch_optional(&mut **tx)
    .await?;
    Ok(printer.filter(|row| row.print_enabled))
}

fn build_final_receipt(receipt: &Receipt, extras: &ReceiptExtras) -> Vec<u8> {
    let mut out = EscPos::new();
    out.init();
    if !receipt.shop_name.is_empty() {
        out.align_center();
        out.text(&receipt.shop_name);
    }
    out.align_center();
    out.bold_underline();
    out.text("Final Bill");
    out.normal();
    out.pair(&format!("Invoice# {}", receipt.bill_no), &receipt.sold_at);
    out.pair(&format!("Cashier: {}", receipt.cashier), "");
    if let Some(customer) = receipt
        .customer
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty())
    {
        out.text(&clip(&format!("Customer: {customer}"), WIDTH));
    }
    out.rule();
    out.bold();
    out.text(&item_header());
    out.normal();
    let mut items_total = 0_i32;
    for line in &receipt.lines {
        out.text(&item_row(&line.name, line.qty, line.rate, line.amount));
        items_total = items_total.saturating_add(line.amount);
    }
    out.rule();
    out.pair(
        &format!("Items ({})", receipt.lines.len()),
        &money(&receipt.currency, items_total),
    );
    out.rule();
    out.bold_underline();
    out.pair("Total", &money(&receipt.currency, receipt.total));
    out.normal();
    out.rule();
    for tender in &receipt.tenders {
        let label = tender_label(&tender.method, &tender.reference);
        out.pair(&label, &money(&receipt.currency, tender.amount));
    }
    if !extras.lines.is_empty() || !extras.qr_payloads.is_empty() {
        out.rule();
        out.normal();
        for line in &extras.lines {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            out.align_center();
            out.text(&clip(trimmed, WIDTH));
        }
        for payload in &extras.qr_payloads {
            let trimmed = payload.trim();
            if trimmed.is_empty() {
                continue;
            }
            out.feed(1);
            out.align_center();
            out.qr(trimmed, 4);
            out.feed(1);
        }
        out.normal();
    }
    out.feed(1);
    out.align_center();
    out.text("Thank you!");
    out.feed(1);
    out.rule();
    out.bold();
    out.align_center();
    out.text("Check Closed");
    out.normal();
    out.align_center();
    out.text(&printed_at());
    out.feed(1);
    out.cut();
    out.into_bytes()
}

fn dispatch(printer: &PrinterRow, bytes: &[u8], kept: &str) -> Result<(), AppError> {
    let result = match printer.printer_kind.as_str() {
        "network" => send_network(&printer.printer_target, printer.printer_port, bytes),
        "serial" => send_serial(&printer.printer_target, printer.printer_port, bytes),
        "usb" => send_usb(&printer.printer_target, bytes),
        other => Err(format!("{other} is not a printer connection.")),
    };
    result.map_err(|detail| {
        AppError::BadRequest(format!("The receipt did not print. {detail} {kept}"))
    })
}

fn send_network(target: &str, port: i32, bytes: &[u8]) -> Result<(), String> {
    let host = target.trim();
    let port = if (1..=65535).contains(&port) {
        port as u16
    } else {
        9100
    };
    if host.is_empty() {
        return Err("Enter the printer address.".into());
    }
    let addr = format!("{host}:{port}");
    let mut targets = addr
        .to_socket_addrs()
        .map_err(|_| "The printer address could not be read.".to_string())?;
    let Some(addr) = targets.next() else {
        return Err("The printer address could not be read.".into());
    };
    let mut stream = TcpStream::connect_timeout(&addr, Duration::from_secs(4))
        .map_err(|_| "The printer did not answer.".to_string())?;
    stream.set_write_timeout(Some(Duration::from_secs(8))).ok();
    stream
        .write_all(bytes)
        .map_err(|_| "The printer stopped while the receipt was sending.".to_string())?;
    Ok(())
}

fn send_serial(target: &str, baud: i32, bytes: &[u8]) -> Result<(), String> {
    let path = target.trim();
    if path.is_empty() {
        return Err("Enter the serial port.".into());
    }
    let baud = if baud > 0 { baud as u32 } else { 9600 };
    let mut port = serialport::new(path, baud)
        .timeout(Duration::from_secs(8))
        .open()
        .map_err(|_| "The serial printer did not open.".to_string())?;
    port.write_all(bytes)
        .map_err(|_| "The serial printer stopped while the receipt was sending.".to_string())?;
    Ok(())
}

fn send_usb(target: &str, bytes: &[u8]) -> Result<(), String> {
    let node = usb_node(target);
    if let Some(path) = node {
        if let Ok(mut file) = std::fs::OpenOptions::new().write(true).open(&path) {
            if file.write_all(bytes).is_ok() {
                return Ok(());
            }
        }
    }
    send_cups(bytes)
}

fn usb_node(target: &str) -> Option<std::path::PathBuf> {
    let target = target.trim();
    if target.starts_with("/dev/") {
        return Some(Path::new(target).to_path_buf());
    }
    for index in 0..4 {
        let path = Path::new(&format!("/dev/usb/lp{index}")).to_path_buf();
        if path.exists() {
            return Some(path);
        }
    }
    None
}

fn send_cups(bytes: &[u8]) -> Result<(), String> {
    let queue = std::env::var("PRINT_CUPS_QUEUE").unwrap_or_else(|_| "Receipt".into());
    let mut child = Command::new("lp")
        .args(["-d", &queue, "-o", "raw", "-t", "Receipt"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|_| "The USB printer queue is not available.".to_string())?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(bytes)
            .map_err(|_| "The USB printer did not accept the receipt.".to_string())?;
    }
    let mut stderr = String::new();
    if let Some(mut err) = child.stderr.take() {
        err.read_to_string(&mut stderr).ok();
    }
    let finished = child
        .wait()
        .map_err(|_| "The USB printer did not accept the receipt.".to_string())?;
    if finished.success() {
        Ok(())
    } else {
        Err(stderr
            .trim()
            .is_empty()
            .then(|| "The USB printer did not accept the receipt.".to_string())
            .unwrap_or_else(|| stderr.trim().to_string()))
    }
}

fn tender_label(method: &str, reference: &str) -> String {
    let name = match method {
        "cash" => "Cash".to_string(),
        "gift.card" => "Gift card".to_string(),
        other => other.replace('.', " "),
    };
    let reference = reference.trim();
    if reference.is_empty() {
        name
    } else {
        format!("{name} ({reference})")
    }
}

fn money(currency: &str, amount: i32) -> String {
    money_wide(currency, i64::from(amount))
}

fn money_wide(currency: &str, amount: i64) -> String {
    format!("{currency}{amount}")
}

struct MixSlipLine {
    name: String,
    qty_in: i64,
    qty_out: i64,
}

fn build_mix_slip(
    shop: &str,
    _currency: &str,
    label: &str,
    lines: &[MixSlipLine],
    truncated: bool,
) -> Vec<u8> {
    let mut out = EscPos::new();
    out.init();
    if !shop.is_empty() {
        out.align_center();
        out.text(shop);
    }
    out.align_center();
    out.bold_underline();
    out.text("Product Mix");
    out.normal();
    out.align_center();
    out.text(label);
    out.rule();
    out.bold();
    out.text(&format!(
        "{}{}{}{}",
        left("Item", 21),
        right("In", 7),
        right("Out", 7),
        right("Net", 7)
    ));
    out.normal();
    let mut total_in = 0_i64;
    let mut total_out = 0_i64;
    if lines.is_empty() {
        out.text("No movement");
    }
    for line in lines {
        let net = line.qty_in - line.qty_out;
        total_in += line.qty_in;
        total_out += line.qty_out;
        out.text(&format!(
            "{}{}{}{}",
            left(&line.name, 21),
            right(&line.qty_in.to_string(), 7),
            right(&line.qty_out.to_string(), 7),
            right(&net.to_string(), 7)
        ));
    }
    out.rule();
    out.bold();
    out.text(&format!(
        "{}{}{}{}",
        left("Total", 21),
        right(&total_in.to_string(), 7),
        right(&total_out.to_string(), 7),
        right(&(total_in - total_out).to_string(), 7)
    ));
    out.normal();
    if truncated {
        out.text("More items are on the screen.");
    }
    out.pair("Items", &lines.len().to_string());
    out.feed(1);
    out.align_center();
    out.text(&format!("Printed {}", printed_at()));
    out.feed_for_cut();
    out.cut();
    out.into_bytes()
}

fn build_cash_slip(shop: &str, currency: &str, shift: &CashClosing) -> Vec<u8> {
    let mut out = EscPos::new();
    out.init();
    if !shop.is_empty() {
        out.align_center();
        out.text(shop);
    }
    out.align_center();
    out.bold_underline();
    out.text("Cash Closing");
    out.normal();
    out.text(&format!("Shift {}", shift.id));
    out.text(&format!("Opened {}", shift.opened_at));
    if let Some(closed) = &shift.closed_at {
        out.text(&format!("Closed {closed}"));
    }
    out.text(&format!("Opened by {}", shift.opened_by));
    if let Some(name) = &shift.closed_by {
        out.text(&format!("Closed by {name}"));
    }
    out.rule();
    out.pair(
        "Previous till",
        &money_wide(currency, i64::from(shift.opening_cash)),
    );
    out.pair(
        "Petty cash",
        &money_wide(currency, i64::from(shift.petty_cash)),
    );
    out.pair("Cash sales", &money_wide(currency, shift.sales));
    if shift.lines.is_empty() {
        out.pair(
            "Expenses",
            &money_wide(currency, i64::from(shift.expenses.unwrap_or(0))),
        );
    } else {
        out.text("Expenses");
        for line in &shift.lines {
            out.pair(
                &format!("{} {}", line.category, line.description),
                &money_wide(currency, i64::from(line.amount)),
            );
        }
        out.pair(
            "Expense total",
            &money_wide(currency, i64::from(shift.expenses.unwrap_or(0))),
        );
    }
    out.pair(
        "Taken out",
        &money_wide(currency, i64::from(shift.cash_taken.unwrap_or(0))),
    );
    out.rule();
    out.bold_underline();
    out.pair(
        "System count",
        &money_wide(currency, shift.system_count.unwrap_or(0)),
    );
    out.pair(
        "Manual count",
        &money_wide(currency, i64::from(shift.manual_count.unwrap_or(0))),
    );
    out.normal();
    out.pair(
        "Difference",
        &money_wide(currency, shift.difference.unwrap_or(0)),
    );
    out.bold();
    out.pair(
        "Left for next",
        &money_wide(currency, i64::from(shift.cash_left.unwrap_or(0))),
    );
    out.normal();
    out.feed(1);
    out.align_center();
    out.text(&format!("Printed {}", printed_at()));
    out.feed_for_cut();
    out.cut();
    out.into_bytes()
}

fn item_header() -> String {
    format!(
        "{}{}{}{}",
        left("Item", 22),
        right("Qty", 3),
        right("Rate", 7),
        right("Ttl", 10)
    )
}

fn item_row(name: &str, qty: i32, rate: i32, amount: i32) -> String {
    format!(
        "{}{}{}{}",
        left(name, 22),
        right(&qty.to_string(), 3),
        right(&rate.to_string(), 7),
        right(&amount.to_string(), 10)
    )
}

fn left(text: &str, width: usize) -> String {
    format!("{:<width$}", clip(text, width))
}

fn right(text: &str, width: usize) -> String {
    format!("{:>width$}", clip(text, width))
}

fn clip(text: &str, width: usize) -> String {
    plain(text).chars().take(width).collect()
}

/// Thermal printers take one byte per character. Swap punctuation they cannot print.
fn plain(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '-' | '–' | '—' | '−' | '―' => out.push('-'),
            '·' | '•' | '∙' | '⋅' => out.push('-'),
            '’' | '‘' | '´' | '`' => out.push('\''),
            '“' | '”' => out.push('"'),
            '\u{00a0}' | '\u{2009}' | '\u{202f}' => out.push(' '),
            c if c.is_ascii() && !c.is_control() => out.push(c),
            c if c.is_control() => {}
            _ => out.push(' '),
        }
    }
    out
}

fn printed_at() -> String {
    chrono::Local::now()
        .format("%m/%d/%Y, %I:%M %p")
        .to_string()
}

struct EscPos {
    bytes: Vec<u8>,
}

impl EscPos {
    fn new() -> Self {
        Self {
            bytes: Vec::with_capacity(1024),
        }
    }

    fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }

    fn init(&mut self) {
        self.bytes.extend_from_slice(b"\x1b\x40");
    }

    fn align_center(&mut self) {
        self.bytes.extend_from_slice(b"\x1b\x61\x01");
    }

    fn bold(&mut self) {
        self.bytes
            .extend_from_slice(b"\x1b\x45\x01\x1b\x35\x1b\x2d\x00");
    }

    fn bold_underline(&mut self) {
        self.bytes
            .extend_from_slice(b"\x1b\x45\x01\x1b\x35\x1b\x2d\x01");
    }

    fn normal(&mut self) {
        self.bytes
            .extend_from_slice(b"\x1b\x45\x00\x1b\x35\x1b\x2d\x00\x1b\x61\x00");
    }

    fn text(&mut self, line: &str) {
        let line = plain(line);
        self.bytes.extend_from_slice(line.as_bytes());
        self.bytes.push(b'\n');
    }

    fn pair(&mut self, left_text: &str, right_text: &str) {
        let half = WIDTH / 2;
        self.text(&format!(
            "{}{}",
            left(left_text, half),
            right(right_text, half)
        ));
    }

    fn rule(&mut self) {
        self.text(&"-".repeat(WIDTH));
    }

    fn feed(&mut self, lines: usize) {
        for _ in 0..lines {
            self.bytes.push(b'\n');
        }
    }

    /// ESC/POS QR Code (GS ( k), model 2. `module` is 1–16 (dot size).
    fn qr(&mut self, payload: &str, module: u8) {
        let data = plain(payload);
        if data.is_empty() {
            return;
        }
        let size = module.clamp(1, 16);
        // Model 2
        self.bytes
            .extend_from_slice(b"\x1d\x28\x6b\x04\x00\x31\x41\x32\x00");
        // Module size
        self.bytes
            .extend_from_slice(&[0x1d, 0x28, 0x6b, 0x03, 0x00, 0x31, 0x43, size]);
        // Error correction M
        self.bytes
            .extend_from_slice(b"\x1d\x28\x6b\x03\x00\x31\x45\x31");
        // Store data: pL pH cn fn m + bytes
        let store_len = data.len() + 3;
        let p_l = (store_len & 0xff) as u8;
        let p_h = ((store_len >> 8) & 0xff) as u8;
        self.bytes
            .extend_from_slice(&[0x1d, 0x28, 0x6b, p_l, p_h, 0x31, 0x50, 0x30]);
        self.bytes.extend_from_slice(data.as_bytes());
        // Print symbol
        self.bytes
            .extend_from_slice(b"\x1d\x28\x6b\x03\x00\x31\x51\x30");
    }

    /// The blade sits above the print head, so the last line has to travel before the cut.
    fn feed_for_cut(&mut self) {
        self.bytes.extend_from_slice(b"\x1b\x64\x0a");
    }

    fn cut(&mut self) {
        self.bytes.extend_from_slice(b"\x1d\x56\x00");
    }
}

#[derive(sqlx::FromRow)]
struct PrinterRow {
    print_enabled: bool,
    printer_kind: String,
    printer_target: String,
    printer_port: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrinterSettings {
    pub enabled: bool,
    pub kind: String,
    pub target: String,
    pub port: i32,
}

#[derive(Debug, Deserialize)]
pub struct PrinterForm {
    #[serde(default)]
    pub print_enabled: bool,
    #[serde(default = "default_printer_kind")]
    pub printer_kind: String,
    #[serde(default)]
    pub printer_target: String,
    #[serde(default = "default_printer_port")]
    pub printer_port: i32,
}

impl PrinterForm {
    pub fn checked(self) -> Result<PrinterSettings, AppError> {
        let kind = self.printer_kind.trim().to_string();
        if !["network", "usb", "serial"].contains(&kind.as_str()) {
            return Err(AppError::BadRequest(
                "The printer is network, usb, or serial.".into(),
            ));
        }
        if !(1..=1_000_000).contains(&self.printer_port) {
            return Err(AppError::BadRequest(
                "The printer port is not a usable number.".into(),
            ));
        }
        let target = self.printer_target.trim().to_string();
        if self.print_enabled && kind != "usb" && target.is_empty() {
            return Err(AppError::BadRequest("Enter the printer address.".into()));
        }
        Ok(PrinterSettings {
            enabled: self.print_enabled,
            kind,
            target,
            port: self.printer_port,
        })
    }
}

fn default_printer_kind() -> String {
    "network".into()
}
fn default_printer_port() -> i32 {
    9100
}

pub async fn load_printer_settings(tx: &mut TenantTx<'_>) -> Result<PrinterSettings, AppError> {
    let row = sqlx::query_as::<_, PrinterRow>(
        "SELECT enabled AS print_enabled, kind AS printer_kind, target AS printer_target, port AS printer_port FROM printing_settings WHERE tenant_id = $1",
    ).bind(tx.tenant_id.as_uuid()).fetch_optional(tx.conn()).await?;
    Ok(row
        .map(|row| PrinterSettings {
            enabled: row.print_enabled,
            kind: row.printer_kind,
            target: row.printer_target,
            port: row.printer_port,
        })
        .unwrap_or(PrinterSettings {
            enabled: false,
            kind: "network".into(),
            target: String::new(),
            port: 9100,
        }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn final_receipt_has_the_bill_and_a_cut() {
        let receipt = Receipt {
            shop_name: "Corner Shop".into(),
            currency: "PKR".into(),
            bill_no: "BILL-1001".into(),
            sold_at: "2026-11-16 08:10 PM".into(),
            cashier: "Kashif".into(),
            customer: Some("Ali".into()),
            lines: vec![ReceiptLine {
                name: "Cigarette · Pack".into(),
                qty: 2,
                rate: 50,
                amount: 100,
            }],
            total: 100,
            tenders: vec![ReceiptTender {
                method: "cash".into(),
                amount: 100,
                reference: String::new(),
            }],
        };
        let bytes = build_final_receipt(&receipt, &ReceiptExtras::default());
        let text = String::from_utf8_lossy(&bytes);
        assert!(text.contains("Corner Shop"));
        assert!(text.contains("Final Bill"));
        assert!(text.contains("Invoice# BILL-1001"));
        assert!(text.contains("Customer: Ali"));
        assert!(text.contains("Cigarette - Pack"));
        assert!(!text.contains('·'));
        assert!(text.contains("PKR100"));
        assert!(text.contains("Cash"));
        assert!(text.contains("Check Closed"));
        assert!(bytes.windows(3).any(|chunk| chunk == b"\x1d\x56\x00"));
    }

    #[test]
    fn final_receipt_prints_plugin_footer_and_qr() {
        let receipt = Receipt {
            shop_name: "Corner Shop".into(),
            currency: "PKR".into(),
            bill_no: "BILL-1001".into(),
            sold_at: "2026-11-16 08:10 PM".into(),
            cashier: "Kashif".into(),
            customer: None,
            lines: vec![],
            total: 0,
            tenders: vec![],
        };
        let extras = ReceiptExtras {
            lines: vec!["FBR Invoice: INV-9".into()],
            qr_payloads: vec!["https://verify.example/INV-9".into()],
        };
        let bytes = build_final_receipt(&receipt, &extras);
        let text = String::from_utf8_lossy(&bytes);
        assert!(text.contains("FBR Invoice: INV-9"));
        assert!(
            bytes
                .windows(8)
                .any(|chunk| chunk == b"\x1d\x28\x6b\x03\x00\x31\x51\x30"),
            "expected ESC/POS QR print command"
        );
        assert!(bytes.windows(b"https://verify.example/INV-9".len()).any(|chunk| {
            chunk == b"https://verify.example/INV-9"
        }));
    }

    #[test]
    fn product_mix_lists_in_and_out() {
        let bytes = build_mix_slip(
            "Corner Shop",
            "PKR",
            "17 Nov 2026 – 17 Nov 2026",
            &[MixSlipLine {
                name: "Pan".into(),
                qty_in: 5,
                qty_out: 2,
            }],
            false,
        );
        let text = String::from_utf8_lossy(&bytes);
        assert!(text.contains("Product Mix"));
        assert!(text.contains("Pan"));
        assert!(text.contains("17 Nov 2026 - 17 Nov 2026"));
        assert!(!text.contains('–'));
        assert!(text.contains("Printed "));
        assert!(bytes.windows(2).any(|chunk| chunk == b"\x1b\x64"));
        assert!(bytes.windows(3).any(|chunk| chunk == b"\x1d\x56\x00"));
    }

    #[test]
    fn cash_closing_lists_the_count() {
        let shift = CashClosing {
            id: 4,
            opened_by: "Kashif".into(),
            closed_by: Some("cashier".into()),
            opened_at: "17 Nov 2026, 09:00 AM".into(),
            closed_at: Some("17 Nov 2026, 11:00 PM".into()),
            opening_cash: 5000,
            petty_cash: 500,
            sales: 12000,
            expenses: Some(200),
            cash_taken: Some(10000),
            manual_count: Some(7300),
            system_count: Some(7300),
            difference: Some(0),
            cash_left: Some(5000),
            lines: vec![cash_contract::CashExpense {
                description: "Tea".into(),
                category: "Food".into(),
                amount: 200,
            }],
        };
        let bytes = build_cash_slip("Corner Shop", "PKR", &shift);
        let text = String::from_utf8_lossy(&bytes);
        assert!(text.contains("Cash Closing"));
        assert!(text.contains("PKR7300"));
        assert!(text.contains("Food Tea"));
        assert!(text.contains("Left for next"));
        assert!(text.contains("Printed "));
        assert!(bytes.windows(2).any(|chunk| chunk == b"\x1b\x64"));
        assert!(bytes.windows(3).any(|chunk| chunk == b"\x1d\x56\x00"));
    }

    #[test]
    fn printer_settings_reject_invalid_connections_and_missing_targets() {
        assert!(PrinterForm {
            print_enabled: true,
            printer_kind: "network".into(),
            printer_target: "  ".into(),
            printer_port: 9100,
        }
        .checked()
        .is_err());
        assert!(PrinterForm {
            print_enabled: false,
            printer_kind: "bluetooth".into(),
            printer_target: String::new(),
            printer_port: 9100,
        }
        .checked()
        .is_err());
        let settings = PrinterForm {
            print_enabled: true,
            printer_kind: " usb ".into(),
            printer_target: " 0x04b8:0x0e28 ".into(),
            printer_port: 1,
        }
        .checked()
        .unwrap();
        assert_eq!(settings.kind, "usb");
        assert_eq!(settings.target, "0x04b8:0x0e28");
    }
}
