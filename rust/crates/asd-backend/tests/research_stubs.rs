//! owner: WP-X.0
//!
//! Phase 8 step 0: the researcher toolkit routes still stubbed (`routes/analysis_runs.rs`,
//! WP-X.3) are mounted with their GOV-1 permission and answer `501 not-implemented` to an
//! allowed role; other roles get 403, another org's path 404, no bearer 401, and the vendor
//! (`superadmin`) is refused the run actions. WP-X.2 filled `routes/research.rs` and the
//! sealed research-log upload; their RBAC and behaviour are tested in `experiments.rs`,
//! `cohorts.rs`, `config_effective.rs` and `research_frames.rs`. Needs `DATABASE_URL`;
//! skipped otherwise.

mod common;

use asd_backend::auth::rbac::{self, Action};
use asd_contract::api_v1::Role;
use common::auth::*;
use common::*;
use http::{Method, StatusCode};
use serde_json::json;
use uuid::Uuid;

const ORG_ROLES: [Role; 6] = [
    Role::Owner,
    Role::Admin,
    Role::Clinician,
    Role::Researcher,
    Role::Reviewer,
    Role::Viewer,
];

/// `(method, path with {org} / {id}, action)`.
const STUBS: &[(Method, &str, Action)] = &[
    (
        Method::POST,
        "/v1/orgs/{org}/analysis-runs",
        Action::AnalysisRun,
    ),
    (
        Method::GET,
        "/v1/orgs/{org}/analysis-runs",
        Action::AnalysisRead,
    ),
    (Method::GET, "/v1/analysis-runs/{id}", Action::AnalysisRead),
    (
        Method::GET,
        "/v1/analysis-runs/{id}/sessions",
        Action::AnalysisRead,
    ),
    (
        Method::POST,
        "/v1/analysis-runs/{id}/cancel",
        Action::AnalysisRun,
    ),
    (
        Method::POST,
        "/v1/analysis-runs/{id}/rerun",
        Action::AnalysisRun,
    ),
];

fn uri(template: &str, org: Uuid) -> String {
    template
        .replace("{org}", &org.to_string())
        .replace("{id}", &Uuid::new_v4().to_string())
}

fn request(method: &Method, uri: &str, bearer: Option<&str>) -> http::Request<axum::body::Body> {
    if *method == Method::GET {
        empty(method.clone(), uri, bearer)
    } else {
        json(method.clone(), uri, &json!({}), bearer)
    }
}

#[tokio::test]
async fn every_stub_is_mounted_behind_its_permission_and_answers_501() {
    let Some(db) = TestDb::migrated().await else {
        return;
    };
    let state = state_with_pool(db.pool.clone());
    let app = router(state.clone());
    let org = seed_org(&db.pool, "Clinic A").await;
    let other = seed_org(&db.pool, "Clinic B").await;

    for role in ORG_ROLES {
        let email = format!("{}-{}@example.org", role_str(role), Uuid::new_v4().simple());
        let user = seed_user(&db.pool, Some(org), role, &email).await;
        let token = access_token(&state, user, Some(org), role);
        for (method, template, action) in STUBS {
            let (status, _, body) = send(
                app.clone(),
                request(method, &uri(template, org), Some(&token)),
            )
            .await;
            if rbac::allows(role, *action) {
                assert_eq!(
                    status,
                    StatusCode::NOT_IMPLEMENTED,
                    "{role:?} {method} {template}"
                );
                assert_eq!(slug(&body), "not-implemented", "{body}");
                assert_eq!(body["status"], 501);
            } else {
                assert_eq!(
                    status,
                    StatusCode::FORBIDDEN,
                    "{role:?} {method} {template}"
                );
            }
            // Org-path routes: another org is a 404 before the role check (AUTH-4).
            if template.contains("{org}") {
                let (status, _, _) = send(
                    app.clone(),
                    request(method, &uri(template, other), Some(&token)),
                )
                .await;
                assert_eq!(
                    status,
                    StatusCode::NOT_FOUND,
                    "{role:?} {method} {template} other org"
                );
            }
        }
    }

    // GOV-1: the vendor reads results and config, but never runs, edits or promotes.
    let root = seed_user(&db.pool, None, Role::Superadmin, "root@example.org").await;
    let token = access_token(&state, root, None, Role::Superadmin);
    for (method, template, action) in STUBS {
        let (status, _, _) = send(
            app.clone(),
            request(method, &uri(template, org), Some(&token)),
        )
        .await;
        let want = if rbac::VENDOR_EXCLUDED_RESEARCH_ACTIONS.contains(action) {
            StatusCode::FORBIDDEN
        } else {
            StatusCode::NOT_IMPLEMENTED
        };
        assert_eq!(status, want, "superadmin {method} {template}");
    }

    // No bearer: 401 everywhere.
    for (method, template, _) in STUBS {
        let (status, _, _) = send(app.clone(), request(method, &uri(template, org), None)).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{method} {template}");
    }
    db.cleanup().await;
}

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
