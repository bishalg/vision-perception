//! owner: WP-B.1
//!
//! HTTP routes. `health` sits at the root (CON-22); everything else lives under `/v1`
//! and is registered in [`v1`] by the package that owns the route module.

use axum::Router;

use crate::app::AppState;
use crate::clinical::reauth::StepUpLimiter;

pub mod analysis_runs; // WP-X.3
pub mod attachments; // WP-B.19 (stub from wave 2 step 0)
pub mod auth;
pub mod blobs;
pub mod care;
pub mod clinical_reviews; // WP-C.10
pub mod config;
pub mod devices;
pub mod encounters; // WP-B.19 (stub from wave 2 step 0)
pub mod exports; // WP-I.6
pub mod follow_ups; // WP-B.19 (stub from wave 2 step 0)
pub mod health;
pub mod integrations; // WP-I.7
pub mod jobs;
pub mod metrics;
pub mod notes;
pub mod orgs;
pub mod participant_detail;
pub mod participants;
pub mod patient_refs;
pub mod privacy; // WP-B.9
pub mod research; // WP-X.2 (stub from phase 8 step 0)
pub mod review;
pub mod sessions;
pub mod share; // WP-I.13 (stub from wave 2 step 0)
pub mod totp; // WP-B.11
pub mod users;
pub mod worklist; // WP-C.10

/// The `/v1` API. Empty in WP-B.1; route packages add their routers here, e.g.
/// `Router::new().nest("/v1", Router::new().merge(auth::router()).merge(sessions::router()))`.
/// Use full route templates (`/sessions/{id}`) so request logs show the template (OBS-1).
pub fn v1() -> Router<AppState> {
    // One SIGN-5 step-up limiter for every route that re-authenticates (sign, cosign,
    // amend, TOTP reset): failures on one consume the budget of the others.
    let step_up_limiter = StepUpLimiter::new();
    let api = Router::new()
        // WP-B.3: session ingest, blob uploads, console reads, review queue.
        .merge(sessions::router())
        .merge(blobs::router())
        .merge(review::router())
        .merge(metrics::router())
        .merge(participants::router())
        .merge(care::router()) // WP-B.6
        .merge(participant_detail::router()) // WP-C.11
        .merge(patient_refs::router()) // WP-C.11
        .merge(notes::router()) // WP-C.11
        .merge(worklist::router()) // WP-C.10
        .merge(clinical_reviews::router(step_up_limiter.clone())) // WP-C.10
        .merge(exports::router()) // WP-I.6
        .merge(integrations::router()) // WP-I.7
        .merge(privacy::router()) // WP-B.9
        // Phase 7 wave 2 (stubs from step 0, filled by their packages): EMR-lite record
        // (WP-B.19) and share-a-case links, public routes included (WP-I.13).
        .merge(encounters::router())
        .merge(attachments::router())
        .merge(follow_ups::router())
        .merge(share::router())
        // Phase 8 step 0 stubs (501 until filled): researcher toolkit experiments, cohorts
        // and effective config (WP-X.2), analysis runs (WP-X.3).
        .merge(research::router())
        .merge(analysis_runs::router())
        // WP-B.5: norm aggregation jobs and runs.
        .merge(jobs::router())
        // WP-B.4: config documents, bundles, device bundle delivery.
        .merge(config::router())
        // WP-B.2: auth, orgs, users, devices (org-scoped routes use `/orgs/{id}/...`).
        .merge(auth::router())
        .merge(orgs::router())
        .merge(users::router(step_up_limiter))
        .merge(totp::router()) // WP-B.11
        .merge(devices::router());
    Router::new().nest("/v1", api)
}
