//! owner: WP-X.3
//!
//! Analysis runs (`docs/spec/modules/asd-research-toolkit/technical.md` RUN-2, RUN-6, RUN-8,
//! RUN-9):
//!
//! - `POST /v1/orgs/{id}/analysis-runs` (`AnalysisRun`): validate, snapshot, insert, enqueue
//!   job kind `analysis_run`, `202 AnalysisRun`;
//! - `GET /v1/orgs/{id}/analysis-runs` and `GET /v1/analysis-runs/{id}` (`AnalysisRead`);
//! - `GET /v1/analysis-runs/{id}/sessions` (`AnalysisRead`, and `SessionRead` in the handler);
//! - `POST /v1/analysis-runs/{id}/cancel` and `.../rerun` (`AnalysisRun`).
//!
//! Phase 8 step 0 stub: every path is mounted with its permission and answers `501
//! not-implemented` until WP-X.3 fills it. `POST /v1/jobs/{id}/cancel` belongs to
//! `routes/jobs.rs` (WP-X.3).

use axum::routing::{get, post};
use axum::Router;

use crate::app::AppState;
use crate::auth::rbac::perm;
use crate::routes::not_implemented;

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/orgs/{id}/analysis-runs",
            post(not_implemented::<perm::AnalysisRun>).get(not_implemented::<perm::AnalysisRead>),
        )
        .route(
            "/analysis-runs/{id}",
            get(not_implemented::<perm::AnalysisRead>),
        )
        .route(
            "/analysis-runs/{id}/sessions",
            get(not_implemented::<perm::AnalysisRead>),
        )
        .route(
            "/analysis-runs/{id}/cancel",
            post(not_implemented::<perm::AnalysisRun>),
        )
        .route(
            "/analysis-runs/{id}/rerun",
            post(not_implemented::<perm::AnalysisRun>),
        )
}
