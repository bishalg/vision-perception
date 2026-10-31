//! owner: WP-B.8 (`session`), WP-I.3 (`clinical` and additions here)
//!
//! FHIR R4 (ADR-0003, FHX-*). [`session`] is the iOS-parity session bundle served by
//! `GET /v1/sessions/{id}/fhir`; its items are re-exported here so callers keep writing
//! `asd_export::fhir::bundle`. [`clinical`] (WP-I.3) extends that bundle into the clinical
//! report bundle and reuses it rather than copying it (FHX-1).

pub mod clinical;
pub mod session;

pub use session::*;
