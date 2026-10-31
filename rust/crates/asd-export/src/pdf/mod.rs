//! owner: WP-B.8 (`diary`, `canvas`), WP-I.5 (`summary`, `letter`, engine and fonts)
//!
//! PDF output (feature `pdf`). [`diary`] is the observation diary for the pediatrician (a
//! port of the iOS `DoctorReportGenerator`, base-14 fonts, deterministic bytes); its items
//! are re-exported here so callers keep writing `asd_export::pdf::render`. [`summary`] and
//! [`letter`] are the PDF/A-2b clinical summary and letter (PDF-1..6, WP-I.5).

mod canvas;
pub mod diary;
pub mod letter;
pub mod summary;

pub use diary::*;
