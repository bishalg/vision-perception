//! owner: WP-B.1
//!
//! HTTP routes. `health` sits at the root (CON-22); everything else lives under `/v1`
//! and is registered in [`v1`] by the package that owns the route module.

use axum::Router;

use crate::app::AppState;

pub mod auth;
pub mod blobs;
pub mod care;
pub mod config;
pub mod devices;
pub mod health;
pub mod jobs;
pub mod metrics;
pub mod orgs;
pub mod participants;
pub mod review;
pub mod sessions;
pub mod users;

/// The `/v1` API. Empty in WP-B.1; route packages add their routers here, e.g.
/// `Router::new().nest("/v1", Router::new().merge(auth::router()).merge(sessions::router()))`.
/// Use full route templates (`/sessions/{id}`) so request logs show the template (OBS-1).
pub fn v1() -> Router<AppState> {
    Router::new()
}
