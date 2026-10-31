//! owner: WP-I.2
//!
//! Tabular exports (CSV-1..4, XLSX-1..3): one column dictionary ([`columns`]) for the CSV
//! dialects ([`csv`]: `csv`, `csv_excel`, `csv_sheets`, ZIP of tables plus `manifest.json`)
//! and the XLSX workbook (`xlsx`, feature `xlsx`). Stub until WP-I.2.

pub mod columns;
pub mod csv;
#[cfg(feature = "xlsx")]
pub mod xlsx;
