//! Landscape letter PDF with a ruled table (Helvetica).

pub fn table_pdf(
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

        stream.push_str("0.6 w 0.15 0.2 0.3 RG 0.95 0.96 0.98 rg\n");
        stream.push_str(&format!(
            "{} {} {} {} re B\n",
            left,
            table_top - table_h,
            table_w,
            table_h
        ));

        stream.push_str("0.89 0.91 0.94 rg\n");
        stream.push_str(&format!(
            "{} {} {} {} re f\n",
            left,
            table_top - header_h,
            table_w,
            header_h
        ));

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
        stream.push_str(&format!(
            "{} {} m {} {} l S\n",
            left,
            table_top - header_h,
            left + table_w,
            table_top - header_h
        ));
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
