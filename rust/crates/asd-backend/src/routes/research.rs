//! owner: WP-X.2
//!
//! Researcher toolkit: experiments, cohorts and the effective configuration
//! (`docs/spec/modules/asd-research-toolkit/technical.md` EXP-2, GOV-2, COH-2, GOV-6):
//!
//! - `POST /v1/orgs/{id}/experiments` (`ExperimentWrite`), `GET /v1/orgs/{id}/experiments`
//!   (`AnalysisRead`), `GET /v1/experiments/{id}` (`AnalysisRead`), `PATCH
//!   /v1/experiments/{id}` (`ExperimentWrite`, `If-Match`), `POST
//!   /v1/experiments/{id}/archive` (`ExperimentWrite`), `POST /v1/experiments/{id}/promote`
//!   (`ExperimentPromote`);
//! - `POST /v1/orgs/{id}/cohorts` (`ExperimentWrite`), `GET /v1/orgs/{id}/cohorts`
//!   (`AnalysisRead`), `GET /v1/cohorts/{id}` (`AnalysisRead`), `PATCH /v1/cohorts/{id}`
//!   (`ExperimentWrite`), `POST /v1/orgs/{id}/cohorts/preview` (`AnalysisRead`);
//! - `GET /v1/orgs/{id}/config/effective` (`ConfigRead`).
//!
//! Phase 8 step 0 stub: every path is mounted with its permission (so RBAC answers 401, 403
//! and 404 as it will) and answers `501 not-implemented` until WP-X.2 fills it. The
//! research frame routes (`GET /v1/sessions/{id}/research-frames`, the sealed `PUT`) live
//! in `routes/sessions.rs` and `routes/blobs.rs` (WP-X.2).

use axum::routing::{get, post};
use axum::Router;

use crate::app::AppState;
use crate::auth::rbac::perm;
use crate::routes::not_implemented;

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/orgs/{id}/experiments",
            post(not_implemented::<perm::ExperimentWrite>)
                .get(not_implemented::<perm::AnalysisRead>),
        )
        .route(
            "/experiments/{id}",
            get(not_implemented::<perm::AnalysisRead>)
                .patch(not_implemented::<perm::ExperimentWrite>),
        )
        .route(
            "/experiments/{id}/archive",
            post(not_implemented::<perm::ExperimentWrite>),
        )
        .route(
            "/experiments/{id}/promote",
            post(not_implemented::<perm::ExperimentPromote>),
        )
        .route(
            "/orgs/{id}/cohorts",
            post(not_implemented::<perm::ExperimentWrite>)
                .get(not_implemented::<perm::AnalysisRead>),
        )
        .route(
            "/orgs/{id}/cohorts/preview",
            post(not_implemented::<perm::AnalysisRead>),
        )
        .route(
            "/cohorts/{id}",
            get(not_implemented::<perm::AnalysisRead>)
                .patch(not_implemented::<perm::ExperimentWrite>),
        )
        .route(
            "/orgs/{id}/config/effective",
            get(not_implemented::<perm::ConfigRead>),
        )
}
