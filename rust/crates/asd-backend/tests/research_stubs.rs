//! owner: WP-X.0
//!
//! Phase 8 step 0 seams that outlive their stubs: the plain proxy blob upload never takes a
//! research log (RFL-3). The routes once stubbed here are filled: `routes/research.rs`
//! (WP-X.2: `experiments.rs`, `cohorts.rs`, `config_effective.rs`, `research_frames.rs`) and
//! `routes/analysis_runs.rs` (WP-X.3: `analysis_runs.rs`, `jobs_cancel.rs`, which also keep
//! the GOV-1 RBAC rows of the analysis routes). Needs `DATABASE_URL`; skipped otherwise.

mod common;

use common::auth::*;
use common::*;
use http::{Method, StatusCode};

#[tokio::test]
async fn the_plain_blob_upload_does_not_take_a_research_log() {
    // RFL-3: research logs reach only the sealed route (WP-X.2), whose rules start with the
    // required digest; the plain proxy upload, which stores bytes as sent, never sees them.
    let Some(db) = TestDb::migrated().await else {
        return;
    };
    // The test config is proxy mode with the memory store.
    let state = state_with_pool(db.pool.clone());
    assert_eq!(state.config.blob_mode, asd_backend::config::BlobMode::Proxy);
    let app = router(state.clone());
    let org = seed_org(&db.pool, "Clinic A").await;
    let (_, device_token) = seed_device(&db.pool, org).await;
    let req = http::Request::builder()
        .method(Method::PUT)
        .uri("/v1/sessions/SESS-1/blobs/research_frames")
        .header(
            http::header::AUTHORIZATION,
            format!("Bearer {device_token}"),
        )
        .header(http::header::CONTENT_TYPE, "application/x-ndjson")
        .body(axum::body::Body::from("{}\n"))
        .unwrap();
    let (status, _, body) = send(app, req).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(slug(&body), "digest-required");
    db.cleanup().await;
}
