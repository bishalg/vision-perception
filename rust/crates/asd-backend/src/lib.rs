//! owner: WP-B.1
//!
//! `asd-backend` library: the Axum application, configuration, database, blob store and
//! telemetry. `main.rs` is a thin CLI (`serve`, `worker`, `migrate`) over this crate so
//! integration tests can build the same router.

pub mod admin;
pub mod app;
pub mod audit;
pub mod auth;
pub mod blob;
pub mod care;
pub mod clinical; // WP-C.10
pub mod config;
pub mod config_docs;
pub mod crypto;
pub mod db;
pub mod error;
pub mod ingest;
pub mod interop; // WP-C.10
pub mod jobs;
pub mod problem;
pub mod read;
pub mod report;
pub mod routes;
pub mod telemetry;
