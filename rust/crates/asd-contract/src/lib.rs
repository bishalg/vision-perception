//! owner: WP-R.0 (module tree), WP-R.3 (contents)
//!
//! Wire contracts, schema export and bundle signing (CON-2..6). The Rust types here are the
//! single source of truth for `docs/spec/schemas/asd/*.json` (`asd-cli schema export`).
//!
//! Feature `validate` adds JSON Schema validation (`schema::validate*`); it is off by
//! default so the mobile library does not link a validator.

pub mod api_v1;
pub mod care;
pub mod clinical;
pub mod config;
mod jcs;
mod mirror;
pub mod problem;
pub mod schema;
pub mod session_v1_6;
pub mod session_v1_7;
pub mod signing;
pub mod tracking_api;
