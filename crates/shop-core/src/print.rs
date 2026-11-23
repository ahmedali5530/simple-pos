use std::io::{Read, Write};
use std::net::{Shutdown, TcpStream, ToSocketAddrs};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

use font8x8::UnicodeFonts;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use crate::error::AppError;
use crate::models::ShopConfig;

fn paper_columns(paper_width_mm: i32) -> usize {
    match paper_width_mm {
        58 => 32,
        72 => 42,
        _ => 48,
    }
}

fn paper_dots(paper_width_mm: i32) -> usize {
    match paper_width_mm {
        58 => 384,
        72 => 512,
        _ => 576,
    }
}

#[derive(Debug, Deserialize)]
pub struct PrintRequest {
    pub bill_no: String,
}

/// Printer values from the settings form. Empty fields fall back to what is
/// already saved, so the test button works before the form is saved.
#[derive(Debug, Default, Deserialize)]
pub struct TestPrintRequest {
    #[serde(default)]
    pub printer_kind: String,
    #[serde(default)]
    pub printer_target: String,
    #[serde(default)]
    pub printer_port: i32,
    #[serde(default)]
    pub printer_command_set: String,
    #[serde(default)]
    pub printer_paper_width: i32,
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

struct Receipt {
    shop_name: String,
    currency: String,
    bill_no: String,
    sold_at: String,
    cashier: String,
    customer: Option<String>,
    lines: Vec<ReceiptLine>,
    total: i32,
}

/// Print a saved bill on the shop printer. A turned-off printer is not an error.
pub async fn print_saved_bill(pool: &PgPool, shop: &ShopConfig, bill_no: &str) -> Result<PrintOutcome, AppError> {
    let bill_no = bill_no.trim();
    if bill_no.is_empty() {
        return Err(AppError::BadRequest("That bill has no number.".into()));
    }
    let Some(printer) = enabled_printer(pool).await? else {
        return Ok(PrintOutcome { printed: false });
    };

    let header = sqlx::query_as::<_, BillHead>(
        r#"
        SELECT
            b.id,
            b.bill_no,
            b.total,
            to_char(b.created_at AT TIME ZONE 'Asia/Karachi', 'YYYY-MM-DD HH12:MI AM') AS sold_at,
            COALESCE(u.name, '') AS cashier,
            b.customer
        FROM bills b
        LEFT JOIN users u ON u.id = b.user_id
        WHERE b.bill_no = $1
        "#,
    )
    .bind(bill_no)
    .fetch_optional(pool)
    .await?;
    let Some(header) = header else {
        return Err(AppError::NotFound("That bill is not in the till.".into()));
    };
    let lines = sqlx::query_as::<_, LineRow>(
        "SELECT name, qty, rate, amount FROM bill_lines WHERE bill_id = $1 ORDER BY id",
    )
    .bind(header.id)
    .fetch_all(pool)
    .await?;
    let shop = crate::billing::current_shop(pool, shop).await?;
    let receipt = Receipt {
        shop_name: shop.name,
        currency: shop.currency,
        bill_no: header.bill_no,
        sold_at: header.sold_at,
        cashier: if header.cashier.is_empty() { "Cashier".into() } else { header.cashier },
        customer: header.customer,
        lines: lines
            .into_iter()
            .map(|line| ReceiptLine {
                name: line.name,
                qty: line.qty,
                rate: line.rate,
                amount: line.amount,
            })
            .collect(),
        total: header.total,
    };
    let bytes = build_final_receipt(
        &receipt,
        CommandSet::parse(&printer.printer_command_set),
        printer.printer_paper_width,
    );
    dispatch(&printer, &bytes, "The sale is saved.")?;
    Ok(PrintOutcome { printed: true })
}

/// Send a small test slip using the values currently in the settings form.
/// Unlike a real receipt this runs even when "print after each sale" is off,
/// so a new printer can be checked before it is enabled.
pub async fn print_test(
    pool: &PgPool,
    shop: &ShopConfig,
    request: TestPrintRequest,
) -> Result<PrintOutcome, AppError> {
    let saved = saved_printer(pool).await?;
    let printer_kind = pick(
        &request.printer_kind,
        saved.as_ref().map(|row| row.printer_kind.as_str()),
        "network",
    );
    if !matches!(printer_kind.as_str(), "network" | "usb" | "serial") {
        return Err(AppError::BadRequest(
            "The printer is network, usb, or serial.".into(),
        ));
    }
    let printer_target = pick(
        &request.printer_target,
        saved.as_ref().map(|row| row.printer_target.as_str()),
        "",
    );
    if printer_kind != "usb" && printer_target.is_empty() {
        return Err(AppError::BadRequest(
            "Enter the printer address before testing.".into(),
        ));
    }
    let printer_port = if (1..=1_000_000).contains(&request.printer_port) {
        request.printer_port
    } else {
        saved.as_ref().map(|row| row.printer_port).unwrap_or(9100)
    };
    let printer_command_set = pick(
        &request.printer_command_set,
        saved.as_ref().map(|row| row.printer_command_set.as_str()),
        "escpos",
    );
    let commands = CommandSet::parse(&printer_command_set);
    if !matches!(
        printer_command_set.as_str(),
        "escpos" | "star" | "star-graphic"
    ) {
        return Err(AppError::BadRequest(
            "The printer commands are ESC/POS, Star, or Star graphic.".into(),
        ));
    }
    let printer_paper_width = if matches!(request.printer_paper_width, 58 | 72 | 80) {
        request.printer_paper_width
    } else {
        saved
            .as_ref()
            .map(|row| row.printer_paper_width)
            .unwrap_or(80)
    };
    if !matches!(printer_paper_width, 58 | 72 | 80) {
        return Err(AppError::BadRequest(
            "The printer paper is 58, 72, or 80 mm.".into(),
        ));
    }
    let printer = PrinterRow {
        print_enabled: true,
        printer_kind,
        printer_target,
        printer_port,
        printer_command_set,
        printer_paper_width,
    };
    let shop = crate::billing::current_shop(pool, shop).await?;
    let bytes = build_test_slip(&shop.name, &shop.currency, &printer, commands);
    dispatch(&printer, &bytes, "Check the address, port, and power.")?;
    Ok(PrintOutcome { printed: true })
}

fn pick(value: &str, saved: Option<&str>, fallback: &str) -> String {
    let value = value.trim();
    if !value.is_empty() {
        return value.to_string();
    }
    saved
        .map(str::trim)
        .filter(|saved| !saved.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| fallback.to_string())
}

pub async fn print_product_mix(
    pool: &PgPool,
    shop: &ShopConfig,
    from: Option<&str>,
    to: Option<&str>,
) -> Result<PrintOutcome, AppError> {
    let Some(printer) = enabled_printer(pool).await? else {
        return Ok(PrintOutcome { printed: false });
    };
    let mix = crate::ledger::product_mix(pool, from, to).await?;
    let shop = crate::billing::current_shop(pool, shop).await?;
    let lines = mix
        .lines
        .iter()
        .map(|line| MixSlipLine {
            name: line.name.clone(),
            qty_in: line.qty_in,
            qty_out: line.qty_out,
        })
        .collect::<Vec<_>>();
    let bytes = build_mix_slip(
        &shop.name,
        &shop.currency,
        &mix.label,
        &lines,
        mix.truncated,
        CommandSet::parse(&printer.printer_command_set),
        printer.printer_paper_width,
    );
    dispatch(&printer, &bytes, "Nothing else changed.")?;
    Ok(PrintOutcome { printed: true })
}

pub async fn print_cash_shift(pool: &PgPool, shop: &ShopConfig, id: i32) -> Result<PrintOutcome, AppError> {
    let Some(printer) = enabled_printer(pool).await? else {
        return Ok(PrintOutcome { printed: false });
    };
    let shift = crate::cash::load_shift(pool, id).await?;
    if shift.closed_at.is_none() {
        return Err(AppError::BadRequest("Close the shift before printing it.".into()));
    }
    let shop = crate::billing::current_shop(pool, shop).await?;
    let bytes = build_cash_slip(
        &shop.name,
        &shop.currency,
        &shift,
        CommandSet::parse(&printer.printer_command_set),
        printer.printer_paper_width,
    );
    dispatch(&printer, &bytes, "The closing is saved.")?;
    Ok(PrintOutcome { printed: true })
}

async fn saved_printer(pool: &PgPool) -> Result<Option<PrinterRow>, AppError> {
    Ok(sqlx::query_as::<_, PrinterRow>(
        r#"
        SELECT print_enabled, printer_kind, printer_target, printer_port, printer_command_set, printer_paper_width
        FROM shop_settings
        WHERE id = 1
        "#,
    )
    .fetch_optional(pool)
    .await?)
}

async fn enabled_printer(pool: &PgPool) -> Result<Option<PrinterRow>, AppError> {
    Ok(saved_printer(pool).await?.filter(|row| row.print_enabled))
}

fn build_final_receipt(receipt: &Receipt, commands: CommandSet, paper_width_mm: i32) -> Vec<u8> {
    let mut out = ReceiptWriter::new(commands, paper_width_mm);
    let columns = out.columns();
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
    if let Some(customer) = receipt.customer.as_deref().map(str::trim).filter(|name| !name.is_empty()) {
        out.text(&clip(&format!("Customer: {customer}"), columns));
    }
    out.rule();
    out.bold();
    out.text(&item_header(columns));
    out.normal();
    let mut items_total = 0_i32;
    for line in &receipt.lines {
        out.text(&item_row(columns, &line.name, line.qty, line.rate, line.amount));
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
    out.pair("Cash", &money(&receipt.currency, receipt.total));
    out.rule();
    out.bold_underline();
    out.pair("Change", &money(&receipt.currency, 0));
    out.normal();
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
    eprintln!(
        "[print] {} {} ({}) {} bytes",
        printer.printer_kind,
        printer.printer_target,
        printer.printer_command_set,
        bytes.len()
    );
    let result = match printer.printer_kind.as_str() {
        "network" => send_network(&printer.printer_target, printer.printer_port, bytes),
        "serial" => send_serial(&printer.printer_target, printer.printer_port, bytes),
        "usb" => send_usb(&printer.printer_target, bytes),
        other => Err(format!("{other} is not a printer connection.")),
    };
    result.map_err(|detail| AppError::BadRequest(format!("The receipt did not print. {detail} {kept}")))
}

fn send_network(target: &str, port: i32, bytes: &[u8]) -> Result<(), String> {
    let host = target.trim();
    let port = if (1..=65535).contains(&port) { port as u16 } else { 9100 };
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
    stream
        .set_write_timeout(Some(Duration::from_secs(8)))
        .ok();
    stream
        .write_all(bytes)
        .map_err(|_| "The printer stopped while the receipt was sending.".to_string())?;
    stream.flush().ok();
    // Half-close after the job so the printer sees a clean FIN. Dropping a
    // socket that still has unread status bytes can send RST instead, and the
    // printer then throws the queued ticket away.
    if stream.shutdown(Shutdown::Write).is_ok() {
        stream
            .set_read_timeout(Some(Duration::from_millis(800)))
            .ok();
        let mut status = [0u8; 256];
        loop {
            match stream.read(&mut status) {
                Ok(0) | Err(_) => break,
                Ok(_) => continue,
            }
        }
    }
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

fn money(currency: &str, amount: i32) -> String {
    money_wide(currency, i64::from(amount))
}

fn money_wide(currency: &str, amount: i64) -> String {
    format!("{currency}{amount}")
}

fn command_label(commands: CommandSet) -> &'static str {
    match commands {
        CommandSet::EscPos => "ESC/POS",
        CommandSet::Star => "Star line",
        CommandSet::StarGraphic => "Star graphic",
    }
}

fn build_test_slip(
    shop: &str,
    currency: &str,
    printer: &PrinterRow,
    commands: CommandSet,
) -> Vec<u8> {
    let mut out = ReceiptWriter::new(commands, printer.printer_paper_width);
    let columns = out.columns();
    out.init();
    if !shop.is_empty() {
        out.align_center();
        out.text(shop);
    }
    out.align_center();
    out.bold_underline();
    out.text("Printer Test");
    out.normal();
    out.text(&format!("Commands: {}", command_label(commands)));
    out.text(&format!(
        "Via: {} {}",
        printer.printer_kind,
        printer.printer_target
    ));
    out.rule();
    out.bold();
    out.text(&item_header(columns));
    out.normal();
    out.text(&item_row(columns, "Test item", 1, 50, 50));
    out.rule();
    out.bold_underline();
    out.pair("Total", &money(currency, 50));
    out.normal();
    out.feed(1);
    out.align_center();
    out.text("If this line is readable, the printer is ready.");
    out.feed(1);
    out.align_center();
    out.text(&printed_at());
    out.feed_for_cut();
    out.cut();
    out.into_bytes()
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
    commands: CommandSet,
    paper_width_mm: i32,
) -> Vec<u8> {
    let mut out = ReceiptWriter::new(commands, paper_width_mm);
    let columns = out.columns();
    let name_width = columns.saturating_sub(21).max(8);
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
        left("Item", name_width),
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
            left(&line.name, name_width),
            right(&line.qty_in.to_string(), 7),
            right(&line.qty_out.to_string(), 7),
            right(&net.to_string(), 7)
        ));
    }
    out.rule();
    out.bold();
    out.text(&format!(
        "{}{}{}{}",
        left("Total", name_width),
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

fn build_cash_slip(
    shop: &str,
    currency: &str,
    shift: &crate::cash::CashShift,
    commands: CommandSet,
    paper_width_mm: i32,
) -> Vec<u8> {
    let mut out = ReceiptWriter::new(commands, paper_width_mm);
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
    out.pair("Previous till", &money_wide(currency, i64::from(shift.opening_cash)));
    out.pair("Petty cash", &money_wide(currency, i64::from(shift.petty_cash)));
    out.pair("Cash sales", &money_wide(currency, shift.sales));
    if shift.lines.is_empty() {
        out.pair("Expenses", &money_wide(currency, i64::from(shift.expenses.unwrap_or(0))));
    } else {
        out.text("Expenses");
        for line in &shift.lines {
            out.pair(
                &format!("{} {}", line.category, line.description),
                &money_wide(currency, i64::from(line.amount)),
            );
        }
        out.pair("Expense total", &money_wide(currency, i64::from(shift.expenses.unwrap_or(0))));
    }
    out.pair("Taken out", &money_wide(currency, i64::from(shift.cash_taken.unwrap_or(0))));
    out.rule();
    out.bold_underline();
    out.pair("System count", &money_wide(currency, shift.system_count.unwrap_or(0)));
    out.pair("Manual count", &money_wide(currency, i64::from(shift.manual_count.unwrap_or(0))));
    out.normal();
    out.pair("Difference", &money_wide(currency, shift.difference.unwrap_or(0)));
    out.bold();
    out.pair("Left for next", &money_wide(currency, i64::from(shift.cash_left.unwrap_or(0))));
    out.normal();
    out.feed(1);
    out.align_center();
    out.text(&format!("Printed {}", printed_at()));
    out.feed_for_cut();
    out.cut();
    out.into_bytes()
}

fn item_header(columns: usize) -> String {
    let name_width = columns.saturating_sub(20).max(8);
    format!(
        "{}{}{}{}",
        left("Item", name_width),
        right("Qty", 3),
        right("Rate", 7),
        right("Ttl", 10)
    )
}

fn item_row(columns: usize, name: &str, qty: i32, rate: i32, amount: i32) -> String {
    let name_width = columns.saturating_sub(20).max(8);
    format!(
        "{}{}{}{}",
        left(name, name_width),
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
    chrono::Local::now().format("%m/%d/%Y, %I:%M %p").to_string()
}

/// The commands the receipt writer puts on the wire.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CommandSet {
    EscPos,
    /// Star Line Mode text (TSP650/TSP700/TSP800/FVP10 and similar).
    Star,
    /// Star Graphic Mode raster image (TSP100 and similar graphic-only printers).
    StarGraphic,
}

impl CommandSet {
    fn parse(value: &str) -> Self {
        match value.trim().to_lowercase().as_str() {
            "star" => Self::Star,
            "star-graphic" | "star_graphic" | "star-graphics" => Self::StarGraphic,
            _ => Self::EscPos,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Align {
    Left,
    Center,
}

struct GraphicLine {
    text: String,
    align: Align,
    bold: bool,
    underline: bool,
}

/// Star Graphic glyphs are 12x24 dots, like a standard thermal font. The
/// translation from the 8x8 source bitmap is nearest-neighbour.
const STAR_GLYPH_W: usize = 12;
const STAR_SRC_W: usize = 8;
const STAR_SCALE_Y: usize = 3;
const STAR_LINE_H: usize = 8 * STAR_SCALE_Y + 4;

struct ReceiptWriter {
    bytes: Vec<u8>,
    commands: CommandSet,
    columns: usize,
    dots: usize,
    align: Align,
    bold: bool,
    underline: bool,
    graphic: Vec<GraphicLine>,
}

impl ReceiptWriter {
    fn new(commands: CommandSet, paper_width_mm: i32) -> Self {
        Self {
            bytes: Vec::with_capacity(1024),
            commands,
            columns: paper_columns(paper_width_mm),
            dots: paper_dots(paper_width_mm),
            align: Align::Left,
            bold: false,
            underline: false,
            graphic: Vec::new(),
        }
    }

    fn columns(&self) -> usize {
        self.columns
    }

    fn into_bytes(self) -> Vec<u8> {
        if self.commands == CommandSet::StarGraphic {
            render_star_graphic(&self.graphic, self.columns, self.dots)
        } else {
            self.bytes
        }
    }

    fn init(&mut self) {
        if self.commands == CommandSet::StarGraphic {
            return;
        }
        // ESC @ starts both text command sets.
        self.bytes.extend_from_slice(b"\x1b\x40");
    }

    fn align_center(&mut self) {
        self.align = Align::Center;
        match self.commands {
            CommandSet::EscPos => self.bytes.extend_from_slice(b"\x1b\x61\x01"),
            // Star Line Mode: ESC GS a n
            CommandSet::Star => self.bytes.extend_from_slice(b"\x1b\x1d\x61\x01"),
            CommandSet::StarGraphic => {}
        }
    }

    fn bold(&mut self) {
        self.bold = true;
        self.underline = false;
        match self.commands {
            CommandSet::EscPos => self.bytes.extend_from_slice(b"\x1b\x45\x01\x1b\x35\x1b\x2d\x00"),
            // ESC E = emphasized, ESC - 0 = underline off.
            CommandSet::Star => self.bytes.extend_from_slice(b"\x1b\x45\x1b\x2d\x00"),
            CommandSet::StarGraphic => {}
        }
    }

    fn bold_underline(&mut self) {
        self.bold = true;
        self.underline = true;
        match self.commands {
            CommandSet::EscPos => self.bytes.extend_from_slice(b"\x1b\x45\x01\x1b\x35\x1b\x2d\x01"),
            CommandSet::Star => self.bytes.extend_from_slice(b"\x1b\x45\x1b\x2d\x01"),
            CommandSet::StarGraphic => {}
        }
    }

    fn normal(&mut self) {
        self.align = Align::Left;
        self.bold = false;
        self.underline = false;
        match self.commands {
            CommandSet::EscPos => self
                .bytes
                .extend_from_slice(b"\x1b\x45\x00\x1b\x35\x1b\x2d\x00\x1b\x61\x00"),
            // ESC F cancels emphasis; ESC GS a 0 returns to left alignment.
            CommandSet::Star => self
                .bytes
                .extend_from_slice(b"\x1b\x46\x1b\x2d\x00\x1b\x1d\x61\x00"),
            CommandSet::StarGraphic => {}
        }
    }

    fn text(&mut self, line: &str) {
        let line = plain(line);
        if self.commands == CommandSet::StarGraphic {
            self.graphic.push(GraphicLine {
                text: line,
                align: self.align,
                bold: self.bold,
                underline: self.underline,
            });
            return;
        }
        self.bytes.extend_from_slice(line.as_bytes());
        self.bytes.push(b'\n');
    }

    fn pair(&mut self, left_text: &str, right_text: &str) {
        let half = self.columns / 2;
        self.text(&format!("{}{}", left(left_text, half), right(right_text, half)));
    }

    fn rule(&mut self) {
        self.text(&"-".repeat(self.columns));
    }

    fn feed(&mut self, lines: usize) {
        if self.commands == CommandSet::StarGraphic {
            for _ in 0..lines {
                self.graphic.push(GraphicLine {
                    text: String::new(),
                    align: Align::Left,
                    bold: false,
                    underline: false,
                });
            }
            return;
        }
        for _ in 0..lines {
            self.bytes.push(b'\n');
        }
    }

    /// The blade sits above the print head, so the last line has to travel before the cut.
    fn feed_for_cut(&mut self) {
        match self.commands {
            CommandSet::EscPos => self.bytes.extend_from_slice(b"\x1b\x64\x0a"),
            // Star's ESC d 3 already feeds to the cutting position.
            CommandSet::Star => {}
            CommandSet::StarGraphic => self.feed(2),
        }
    }

    fn cut(&mut self) {
        match self.commands {
            CommandSet::EscPos => self.bytes.extend_from_slice(b"\x1d\x56\x00"),
            // ESC d 3 = feed to the cutting position, then a partial cut.
            CommandSet::Star => self.bytes.extend_from_slice(b"\x1b\x64\x33"),
            // Star Graphic Mode cuts when ESC * r B ends the raster document.
            CommandSet::StarGraphic => {}
        }
    }
}

fn render_star_graphic(lines: &[GraphicLine], columns: usize, dots: usize) -> Vec<u8> {
    let content_dots = columns * STAR_GLYPH_W;
    let margin = dots.saturating_sub(content_dots) / 2;
    let bytes_per_row = dots / 8;
    let mut rows: Vec<Vec<u8>> = Vec::with_capacity(lines.len() * STAR_LINE_H);
    for line in lines {
        let chars: Vec<char> = line.text.chars().collect();
        let text_width = chars.len() * STAR_GLYPH_W;
        let start_x = margin
            + if line.align == Align::Center && text_width < content_dots {
                (content_dots - text_width) / 2
            } else {
                0
            };
        let mut canvas = vec![vec![false; dots]; STAR_LINE_H];
        for (index, ch) in chars.iter().enumerate() {
            let Some(glyph) = font8x8::BASIC_FONTS.get(*ch) else {
                continue;
            };
            for (glyph_y, bits) in glyph.iter().enumerate() {
                for dest_x in 0..STAR_GLYPH_W {
                    let source_x = dest_x * STAR_SRC_W / STAR_GLYPH_W;
                    if bits & (1 << source_x) == 0 {
                        continue;
                    }
                    let x = start_x + index * STAR_GLYPH_W + dest_x;
                    if x >= dots {
                        continue;
                    }
                    for stretch in 0..STAR_SCALE_Y {
                        let y = glyph_y * STAR_SCALE_Y + stretch;
                        if y >= STAR_LINE_H {
                            continue;
                        }
                        canvas[y][x] = true;
                        if line.bold && x + 1 < dots {
                            canvas[y][x + 1] = true;
                        }
                    }
                }
            }
        }
        if line.underline {
            let y = 8 * STAR_SCALE_Y - 2;
            let end = (start_x + text_width).min(dots);
            for pixel in canvas[y][start_x..end].iter_mut() {
                *pixel = true;
            }
        }
        for canvas_row in canvas {
            let mut packed = vec![0u8; bytes_per_row];
            for (x, black) in canvas_row.iter().enumerate() {
                if *black {
                    packed[x / 8] |= 0x80 >> (x % 8);
                }
            }
            rows.push(packed);
        }
    }
    if rows.is_empty() {
        rows.push(vec![0u8; bytes_per_row]);
    }
    encode_star_graphic(&rows)
}

fn encode_star_graphic(rows: &[Vec<u8>]) -> Vec<u8> {
    let bytes_per_row = rows.first().map(Vec::len).unwrap_or(0);
    let mut out = Vec::with_capacity(64 + rows.len() * (bytes_per_row + 3));
    out.extend_from_slice(b"\x1b\x2a\x72\x52"); // ESC * r R
    out.extend_from_slice(b"\x1b\x2a\x72\x41"); // ESC * r A
    out.extend_from_slice(b"\x1b\x2a\x72\x50\x30\x00"); // ESC * r P 0
    out.extend_from_slice(b"\x1b\x2a\x72\x45\x31\x33\x00"); // ESC * r E 13
    let n1 = (bytes_per_row & 0xff) as u8;
    let n2 = (bytes_per_row >> 8) as u8;
    for row in rows {
        out.push(b'b');
        out.push(n1);
        out.push(n2);
        out.extend_from_slice(row);
    }
    out.extend_from_slice(b"\x1b\x2a\x72\x42"); // ESC * r B
    out
}

#[derive(sqlx::FromRow)]
struct PrinterRow {
    print_enabled: bool,
    printer_kind: String,
    printer_target: String,
    printer_port: i32,
    printer_command_set: String,
    printer_paper_width: i32,
}

#[derive(sqlx::FromRow)]
struct BillHead {
    id: i32,
    bill_no: String,
    total: i32,
    sold_at: String,
    cashier: String,
    customer: Option<String>,
}

#[derive(sqlx::FromRow)]
struct LineRow {
    name: String,
    qty: i32,
    rate: i32,
    amount: i32,
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
        };
        let bytes = build_final_receipt(&receipt, CommandSet::EscPos, 80);
        let text = String::from_utf8_lossy(&bytes);
        assert!(text.contains("Corner Shop"));
        assert!(text.contains("Final Bill"));
        assert!(text.contains("Invoice# BILL-1001"));
        assert!(text.contains("Customer: Ali"));
        assert!(text.contains("Cigarette - Pack"));
        assert!(!text.contains('·'));
        assert!(text.contains("PKR100"));
        assert!(text.contains("Check Closed"));
        assert!(bytes.windows(3).any(|chunk| chunk == b"\x1d\x56\x00"));
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
            CommandSet::EscPos,
            80,
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
        let shift = crate::cash::CashShift {
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
            lines: vec![crate::cash::CashExpense {
                description: "Tea".into(),
                category: "Food".into(),
                amount: 200,
            }],
        };
        let bytes = build_cash_slip("Corner Shop", "PKR", &shift, CommandSet::EscPos, 80);
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
    fn star_receipt_uses_star_commands() {
        let receipt = Receipt {
            shop_name: "Corner Shop".into(),
            currency: "PKR".into(),
            bill_no: "BILL-1001".into(),
            sold_at: "2026-11-16 08:10 PM".into(),
            cashier: "Kashif".into(),
            customer: None,
            lines: vec![ReceiptLine {
                name: "Pan".into(),
                qty: 1,
                rate: 50,
                amount: 50,
            }],
            total: 50,
        };
        let bytes = build_final_receipt(&receipt, CommandSet::Star, 80);
        let text = String::from_utf8_lossy(&bytes);
        assert!(text.contains("Corner Shop"));
        assert!(text.contains("Final Bill"));
        assert!(bytes.windows(4).any(|chunk| chunk == b"\x1b\x1d\x61\x01"), "Star center");
        assert!(bytes.windows(2).any(|chunk| chunk == b"\x1b\x45"), "Star emphasis");
        assert!(bytes.windows(3).any(|chunk| chunk == b"\x1b\x64\x33"), "Star partial cut");
        assert!(!bytes.windows(3).any(|chunk| chunk == b"\x1d\x56\x00"), "no ESC/POS cut");
    }

    #[test]
    fn star_graphic_receipt_is_a_raster_document() {
        let receipt = Receipt {
            shop_name: "Corner Shop".into(),
            currency: "PKR".into(),
            bill_no: "BILL-1001".into(),
            sold_at: "2026-11-16 08:10 PM".into(),
            cashier: "Kashif".into(),
            customer: None,
            lines: vec![ReceiptLine {
                name: "Pan".into(),
                qty: 1,
                rate: 50,
                amount: 50,
            }],
            total: 50,
        };
        let bytes = build_final_receipt(&receipt, CommandSet::StarGraphic, 80);
        assert!(bytes.starts_with(b"\x1b\x2a\x72\x52"));
        assert!(bytes
            .windows(7)
            .any(|chunk| chunk == b"\x1b\x2a\x72\x45\x31\x33\x00"));
        assert!(bytes.windows(4).any(|chunk| chunk == b"\x1b\x2a\x72\x42"));
        assert!(bytes
            .windows(3)
            .any(|chunk| chunk == [0x62, 72, 0x00]));
        assert!(!String::from_utf8_lossy(&bytes).contains("Final Bill"));
    }

    #[test]
    fn star_graphic_glyph_is_not_mirrored() {
        let bytes = render_star_graphic(
            &[GraphicLine {
                text: "A".into(),
                align: Align::Left,
                bold: false,
                underline: false,
            }],
            48,
            576,
        );
        let header = bytes
            .windows(3)
            .position(|chunk| chunk == [0x62, 72, 0x00])
            .expect("raster row header");
        let first = &bytes[header + 3..header + 3 + 72];
        // The 8x8 'A' starts at column 2 of its cell and is stretched to 12 dots.
        assert_eq!(first[0] & 0xE0, 0x00, "nothing in the first three dots");
        assert_eq!(first[0] & 0x1C, 0x1C, "the glyph starts at dot 3");
    }

    #[test]
    fn test_slip_uses_the_selected_commands() {
        let printer = PrinterRow {
            print_enabled: true,
            printer_kind: "network".into(),
            printer_target: "192.168.1.11".into(),
            printer_port: 9100,
            printer_command_set: "star".into(),
            printer_paper_width: 80,
        };
        let text = build_test_slip("Corner Shop", "PKR", &printer, CommandSet::Star);
        let text = String::from_utf8_lossy(&text);
        assert!(text.contains("Printer Test"));
        assert!(text.contains("Commands: Star line"));
        assert!(text.contains("Via: network 192.168.1.11"));

        let raster = build_test_slip("Corner Shop", "PKR", &printer, CommandSet::StarGraphic);
        assert!(raster.starts_with(b"\x1b\x2a\x72\x52"));
        assert!(raster.windows(4).any(|chunk| chunk == b"\x1b\x2a\x72\x42"));
    }

    #[test]
    fn paper_width_changes_columns_and_raster_bytes() {
        assert_eq!(paper_columns(58), 32);
        assert_eq!(paper_columns(72), 42);
        assert_eq!(paper_columns(80), 48);
        assert_eq!(paper_dots(58), 384);
        assert_eq!(paper_dots(72), 512);
        assert_eq!(paper_dots(80), 576);

        let receipt = Receipt {
            shop_name: "Corner Shop".into(),
            currency: "PKR".into(),
            bill_no: "BILL-1001".into(),
            sold_at: "2026-11-16 08:10 PM".into(),
            cashier: "Kashif".into(),
            customer: None,
            lines: vec![ReceiptLine {
                name: "Pan".into(),
                qty: 1,
                rate: 50,
                amount: 50,
            }],
            total: 50,
        };
        let wide = build_final_receipt(&receipt, CommandSet::EscPos, 80);
        assert!(String::from_utf8_lossy(&wide).contains(&"-".repeat(48)));
        let narrow = build_final_receipt(&receipt, CommandSet::StarGraphic, 72);
        assert!(narrow
            .windows(3)
            .any(|chunk| chunk == [0x62, 64, 0x00]), "72 mm = 64 bytes/row");
    }

    #[test]
    fn command_set_parses_star() {
        assert_eq!(CommandSet::parse("star"), CommandSet::Star);
        assert_eq!(CommandSet::parse(" STAR "), CommandSet::Star);
        assert_eq!(CommandSet::parse("star-graphic"), CommandSet::StarGraphic);
        assert_eq!(CommandSet::parse("star_graphic"), CommandSet::StarGraphic);
        assert_eq!(CommandSet::parse("escpos"), CommandSet::EscPos);
        assert_eq!(CommandSet::parse(""), CommandSet::EscPos);
    }
}
