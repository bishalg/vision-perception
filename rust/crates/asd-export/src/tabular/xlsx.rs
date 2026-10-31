//! owner: WP-I.2
//!
//! XLSX workbook with `rust_xlsxwriter` (XLSX-1..3, feature `xlsx`): `README`, `Sessions`,
//! `Clips`, `MetricRows`, `Reviews`; watermark in document properties and printed
//! header/footer; 200 000 rows per sheet at most.
//!
//! Text goes through `write_string`, which stores a shared string and never parses a
//! formula, so no `'` guard is applied here (that is a CSV concern). Dates are Excel date
//! cells holding the organisation's local wall-clock time, with the UTC offset in the next
//! column. A de-identified `YYYY-MM` stays text. The creation time in the document
//! properties is the request time, not the clock (XC-8).

use chrono::{Datelike, NaiveDateTime, Timelike};
use rust_xlsxwriter::{
    DocProperties, ExcelDateTime, Format, FormatAlign, Workbook, Worksheet, XlsxError,
};

use super::columns::{rows, Cell, Table};
use super::{TabularOptions, ROW_LIMIT};
use crate::model::ExportRecord;
use crate::watermark::MANDATORY_SENTENCE;
use crate::ExportError;

/// Excel's cell limit is 32 767 characters; stay below it (UTF-16 units) so a long clinical
/// text can never make the writer fail.
const MAX_TEXT_CHARS: usize = 32_000;
/// Excel's header/footer limit is 255 characters including control codes.
const MAX_HF_LINE: usize = 190;

fn err(e: XlsxError) -> ExportError {
    ExportError::Tabular(format!("xlsx: {e}"))
}

fn excel_time(t: &NaiveDateTime) -> Result<ExcelDateTime, XlsxError> {
    let year = u16::try_from(t.year()).unwrap_or(1900);
    // Month, day, minute and second are in range by construction.
    ExcelDateTime::from_ymd(year, t.month() as u8, t.day() as u8)?.and_hms(
        t.hour() as u16,
        t.minute() as u8,
        t.second() as u8,
    )
}

fn clip(s: &str) -> &str {
    match s.char_indices().nth(MAX_TEXT_CHARS) {
        Some((i, _)) => &s[..i],
        None => s,
    }
}

/// Header/footer control codes: `&` is doubled; the text is cut to fit.
fn hf(s: &str) -> String {
    s.chars()
        .take(MAX_HF_LINE)
        .collect::<String>()
        .replace('&', "&&")
}

fn write_cell(
    sheet: &mut Worksheet,
    row: u32,
    col: u16,
    cell: &Cell,
    date: &Format,
) -> Result<(), XlsxError> {
    match cell {
        Cell::Empty => {}
        Cell::Text(s) | Cell::Verbatim(s) => {
            sheet.write_string(row, col, clip(s))?;
        }
        Cell::Int(i) => {
            // Exact up to 2^53, far above any count here.
            sheet.write_number(row, col, *i as f64)?;
        }
        Cell::Float(f) => {
            sheet.write_number(row, col, *f)?;
        }
        Cell::Bool(b) => {
            sheet.write_boolean(row, col, *b)?;
        }
        Cell::Time(t) => {
            sheet.write_datetime_with_format(row, col, excel_time(&t.local)?, date)?;
        }
    }
    Ok(())
}

fn count(records: &[ExportRecord], table: Table) -> usize {
    match table {
        Table::Sessions => records.len(),
        Table::Clips => records.iter().map(|r| r.clips.len()).sum(),
        Table::MetricRows => records
            .iter()
            .flat_map(|r| &r.clips)
            .map(|k| k.rows.len())
            .sum(),
        Table::Reviews => records.iter().filter(|r| r.review.is_some()).count(),
    }
}

/// The workbook (XLSX-1). Refuses more than [`ROW_LIMIT`] rows in any sheet (XLSX-3).
pub fn encode(records: &[ExportRecord], options: &TabularOptions) -> Result<Vec<u8>, ExportError> {
    let records = options.prepare(records)?;
    for table in Table::ALL {
        let rows = count(&records, table);
        if rows > ROW_LIMIT {
            return Err(ExportError::ExportTooLarge {
                rows,
                limit: ROW_LIMIT,
            });
        }
    }

    let wm = &options.watermark;
    let line = wm.line();
    let requested = chrono::DateTime::parse_from_rfc3339(&wm.requested_at_utc)
        .map_err(|_| ExportError::InvalidTimestamp {
            field: "watermark.requested_at_utc",
        })?
        .naive_utc();

    let bold = Format::new().set_bold();
    let head = Format::new().set_bold().set_align(FormatAlign::Left);
    let date = Format::new().set_num_format("yyyy-mm-dd hh:mm:ss");

    let mut book = Workbook::new();
    book.set_properties(
        &DocProperties::new()
            .set_title(format!("Export {}", wm.export_id))
            .set_subject(&line)
            .set_comment(format!("{line}\n{MANDATORY_SENTENCE}"))
            .set_creation_datetime(&excel_time(&requested).map_err(err)?),
    );

    let header = format!("&C{}", hf(MANDATORY_SENTENCE));
    let footer = format!("&L{}&RPage &P of &N", hf(&line));

    // README: watermark, mandatory sentence, column dictionary.
    {
        let s = book.add_worksheet();
        s.set_name("README").map_err(err)?;
        s.set_header(&header);
        s.set_footer(&footer);
        s.write_string_with_format(0, 0, clip(&line), &bold)
            .map_err(err)?;
        s.write_string(1, 0, MANDATORY_SENTENCE).map_err(err)?;
        s.write_string(
            2,
            0,
            "Dates are local time in the organisation time zone; the next column holds the UTC offset.",
        )
        .map_err(err)?;
        for (c, h) in ["sheet", "column", "description"].iter().enumerate() {
            s.write_string_with_format(4, c as u16, *h, &head)
                .map_err(err)?;
        }
        let mut r = 5u32;
        for table in Table::ALL {
            for col in table.columns() {
                s.write_string(r, 0, table.sheet_name()).map_err(err)?;
                s.write_string(r, 1, col.name).map_err(err)?;
                s.write_string(r, 2, col.description).map_err(err)?;
                r += 1;
            }
        }
        s.set_column_width(0, 14).map_err(err)?;
        s.set_column_width(1, 26).map_err(err)?;
        s.set_column_width(2, 90).map_err(err)?;
    }

    for table in Table::ALL {
        let data = rows(table, &wm.export_id, &records)?;
        let s = book.add_worksheet();
        s.set_name(table.sheet_name()).map_err(err)?;
        s.set_header(&header);
        s.set_footer(&footer);
        s.set_freeze_panes(1, 0).map_err(err)?;
        for (c, col) in table.columns().iter().enumerate() {
            let c = c as u16;
            s.write_string_with_format(0, c, col.name, &head)
                .map_err(err)?;
            let width = (col.name.chars().count() + 2).max(12) as f64;
            s.set_column_width(c, width).map_err(err)?;
        }
        for (i, row) in data.iter().enumerate() {
            for (c, cell) in row.iter().enumerate() {
                write_cell(s, i as u32 + 1, c as u16, cell, &date).map_err(err)?;
            }
        }
    }
    book.save_to_buffer().map_err(err)
}
