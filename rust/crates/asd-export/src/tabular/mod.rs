//! owner: WP-I.2
//!
//! Tabular exports (CSV-1..4, XLSX-1..3): one column dictionary ([`columns`]) for the CSV
//! dialects ([`csv`]: `csv`, `csv_excel`, `csv_sheets`, ZIP of tables plus `manifest.json`)
//! and the XLSX workbook (`xlsx`, feature `xlsx`).
//!
//! Inputs are the eligible [`ExportRecord`]s (the backend ran [`crate::policy`]); when the
//! watermark's mode is `deidentified` the encoders apply [`crate::deid::apply`] themselves
//! (XC-4), so no caller can forget it. Output is deterministic: no clock, no randomness, and
//! ZIP/XLSX timestamps are fixed (XC-8).

use std::borrow::Cow;

use crate::deid::{self, SALT_LEN};
use crate::model::ExportRecord;
use crate::policy::Mode;
use crate::watermark::Watermark;
use crate::ExportError;

pub mod columns;
pub mod csv;
#[cfg(feature = "xlsx")]
pub mod xlsx;

/// Rows per sheet or table above which an XLSX export is refused (XLSX-3).
pub const ROW_LIMIT: usize = 200_000;

/// CSV dialect (CSV-1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Dialect {
    /// `csv`: RFC 4180, UTF-8 without BOM, LF, RFC 3339 UTC timestamps, `true`/`false`.
    Canonical,
    /// `csv_excel`: UTF-8 with BOM, CRLF, local `YYYY-MM-DD HH:MM:SS` plus `utc_offset`.
    Excel,
    /// `csv_sheets`: UTF-8 without BOM, LF, local timestamps, `TRUE`/`FALSE`.
    Sheets,
}

impl Dialect {
    /// Name used in `manifest.json`.
    pub fn name(self) -> &'static str {
        match self {
            Self::Canonical => "csv",
            Self::Excel => "csv_excel",
            Self::Sheets => "csv_sheets",
        }
    }
}

/// Options shared by every tabular encoder.
#[derive(Debug, Clone)]
pub struct TabularOptions {
    /// Printed in the manifest and the README, document properties and page headers (XC-6).
    pub watermark: Watermark,
    /// Per-export salt for the pseudonyms. Used only when `watermark.mode` is
    /// `deidentified`; the caller draws it and discards it after the job (XC-4).
    pub deid_salt: [u8; SALT_LEN],
}

impl TabularOptions {
    pub fn new(watermark: Watermark, deid_salt: [u8; SALT_LEN]) -> Self {
        Self {
            watermark,
            deid_salt,
        }
    }

    /// The records as they may be exported: de-identified when the mode says so.
    pub(crate) fn prepare<'a>(
        &self,
        records: &'a [ExportRecord],
    ) -> Result<Cow<'a, [ExportRecord]>, ExportError> {
        match self.watermark.mode {
            Mode::Identified => Ok(Cow::Borrowed(records)),
            Mode::Deidentified => Ok(Cow::Owned(deid::apply(records, &self.deid_salt)?)),
        }
    }
}
