//! owner: WP-R.0
//!
//! ASD scoring core. Pure data and maths: no I/O, no async, no FFI.
//! Domain types (CON-1) live in [`types`]; each logic module is filled by its own work package.

pub mod aggregate;
pub mod baselines;
pub mod flag;
pub mod metrics;
pub mod norms;
pub mod quality;
pub mod scoring;
#[cfg(feature = "synth")]
pub mod synth;
pub mod types;
