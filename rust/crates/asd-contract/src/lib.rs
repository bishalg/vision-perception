//! owner: WP-R.0 (module tree), WP-R.3 (contents)
//!
//! Wire contracts, schema export and bundle signing (CON-2..6). The Rust types here are the
//! single source of truth for `docs/spec/schemas/asd/*.json` (`asd-cli schema export`).
//!
//! Feature `validate` adds JSON Schema validation (`schema::validate*`); it is off by
//! default so the mobile library does not link a validator.

pub mod api_v1;
pub mod auth_policy; // WP-B.16
pub mod care;
pub mod catalog_checks; // WP-C.4
pub mod claims; // WP-L.0 (I18N-3 rule 7)
pub mod clinical;
pub mod config;
pub mod emr; // WP-R.7
pub mod export;
pub mod i18n; // WP-L.0 (I18N-1..5, I18N-11, I18N-12)
mod jcs;
mod mirror;
pub mod params; // WP-X.0 (PRM-1..4)
pub mod privacy; // WP-B.9
pub mod problem;
pub mod profile_checks; // WP-C.5
pub mod research; // WP-X.0 (EXP, COH, RUN, GOV-6 DTOs)
pub mod schema;
pub mod session_v1_6;
pub mod session_v1_7;
pub mod session_v1_8;
pub mod signing;
pub mod tracking_api;
