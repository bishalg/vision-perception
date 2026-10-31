//! owner: WP-B.8
//!
//! Server-side reports of a stored session (API-2):
//! - `GET /v1/sessions/{id}/report.pdf` regenerates the observation diary from
//!   `sessions.summary` with `asd-export` (the device-uploaded copy stays available as
//!   `?source=device`, see `blobs.rs`);
//! - `GET /v1/sessions/{id}/fhir` returns the FHIR R4 collection bundle with the PDF as a
//!   `DocumentReference`.
//!
//! Both are byte-identical to what `asd-export` builds from the same record (parity with
//! iOS is proven in `asd-export`'s golden tests), refuse synthetic sessions (409
//! `capture-source-rejected`, SIM-5) and answer 404 for another org's session (AUTH-4).
//! Needs `DATABASE_URL`; skipped otherwise.

mod common;
mod ingest_support;

use asd_contract::session_v1_6::GazeSessionRecord;
use asd_export::fhir::{self, BundleInput, FhirDevice, FhirSubject, ReportDocument};
use asd_export::pdf::{self, DoctorReportInput};
use common::{send, TestDb};
use http::StatusCode;
use ingest_support::*;
use serde_json::{json, Value};
use uuid::Uuid;

const SESSION: &str = "SESS-GOLDEN-0001";

struct Fixture {
    db: TestDb,
    org: Uuid,
    device: String,
    router: axum::Router,
}

impl Fixture {
    async fn new(allow_synthetic: bool) -> Option<Self> {
        let db = TestDb::migrated().await?;
        let org = seed_quiet_org(&db.pool).await;
        // Registered with app_version "1.0" (ingest_support::seed_device).
        let (_, device) = seed_device(&db.pool, org).await;
        let router = app(state_with(
            db.pool.clone(),
            std::sync::Arc::new(asd_backend::blob::MemoryBlobStore::new()),
            config(allow_synthetic),
        ));
        Some(Self {
            db,
            org,
            device,
            router,
        })
    }

    async fn user(&self, role: &str) -> String {
        seed_user(&self.db.pool, Some(self.org), role).await.1
    }
}

fn golden_record() -> GazeSessionRecord {
    serde_json::from_value(golden()["rawPayload"].clone()).expect("rawPayload")
}

/// What the server must render for the golden session: the token only (CNS-7), no age
/// (the participant has no enrolment bracket), no M-CHAT and no home-routine data.
fn expected_pdf(record: &GazeSessionRecord) -> Vec<u8> {
    pdf::render(&DoctorReportInput {
        participant_label: record.participant_id.clone(),
        child_age_months: None,
        guardian_name: pdf::DEFAULT_GUARDIAN.into(),
        assessment_date: record.started_at.clone(),
        gaze_record: Some(record),
        mchat: None,
        care: None,
    })
    .expect("render")
}

#[tokio::test]
async fn reports_without_user_identity_are_401() {
    for uri in [
        "/v1/sessions/S/report.pdf",
        "/v1/sessions/S/report.pdf?source=device",
        "/v1/sessions/S/fhir",
    ] {
        let (status, _, body) = send(app(common::state_without_db()), common::get(uri)).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{uri}");
        assert_eq!(problem_slug(&body), "unauthorized", "{uri}");
    }
}

#[tokio::test]
async fn report_pdf_is_regenerated_from_the_stored_record() {
    let Some(f) = Fixture::new(false).await else {
        return;
    };
    ingest_ok(&f.router, &f.device, &golden()).await;
    let user = f.user("viewer").await;

    let (status, headers, bytes) = send_raw(
        f.router.clone(),
        user_get(&user, &format!("/v1/sessions/{SESSION}/report.pdf")),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(common::content_type(&headers), "application/pdf");
    assert_eq!(headers[http::header::CACHE_CONTROL], "private, no-store");
    assert_eq!(bytes, expected_pdf(&golden_record()));
    let text = String::from_utf8_lossy(&bytes);
    assert!(text.contains("(This is not a diagnostic tool.) Tj"));
    assert!(text.contains("(child-SESS-GOL) Tj"), "token, never a name");

    // Deterministic: the same session renders the same bytes.
    let (_, _, again) = send_raw(
        f.router.clone(),
        user_get(&user, &format!("/v1/sessions/{SESSION}/report.pdf")),
    )
    .await;
    assert_eq!(again, bytes);
    f.db.cleanup().await;
}

#[tokio::test]
async fn fhir_bundle_matches_the_report_builder() {
    let Some(f) = Fixture::new(false).await else {
        return;
    };
    ingest_ok(&f.router, &f.device, &golden()).await;
    let user = f.user("clinician").await;

    let (status, headers, bytes) = send_raw(
        f.router.clone(),
        user_get(&user, &format!("/v1/sessions/{SESSION}/fhir")),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(common::content_type(&headers), "application/fhir+json");
    assert_eq!(headers[http::header::CACHE_CONTROL], "private, no-store");
    let body: Value = serde_json::from_slice(&bytes).expect("json");

    let record = golden_record();
    let pdf = expected_pdf(&record);
    let expected = fhir::bundle(&BundleInput {
        record: &record,
        subject: FhirSubject::new(&record.participant_id, None, None),
        device: FhirDevice::new(fhir::DEFAULT_DEVICE_NAME, "1.0"),
        mchat: None,
        document: Some(ReportDocument { pdf: &pdf }),
    })
    .unwrap();
    assert_eq!(body, expected);

    let types: Vec<&str> = body["entry"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["resource"]["resourceType"].as_str().unwrap())
        .collect();
    assert_eq!(
        types,
        [
            "Patient",
            "RelatedPerson",
            "Device",
            "Observation",
            "Provenance",
            "DocumentReference"
        ]
    );
    assert_eq!(body["entry"][2]["resource"]["version"][0]["value"], "1.0");
    assert_eq!(
        body["entry"][3]["resource"]["identifier"][0]["value"],
        SESSION
    );
    f.db.cleanup().await;
}

#[tokio::test]
async fn synthetic_sessions_are_refused() {
    let Some(f) = Fixture::new(true).await else {
        return;
    };
    let mut body = golden();
    body["captureSource"] = json!("synthetic");
    body["rawPayload"]["captureSource"] = json!("synthetic");
    ingest_ok(&f.router, &f.device, &body).await;
    let user = f.user("owner").await;

    for path in ["report.pdf", "fhir"] {
        let (status, _, problem) = send(
            f.router.clone(),
            user_get(&user, &format!("/v1/sessions/{SESSION}/{path}")),
        )
        .await;
        assert_eq!(status, StatusCode::CONFLICT, "{path}: {problem}");
        assert_eq!(problem_slug(&problem), "capture-source-rejected", "{path}");
        assert_eq!(
            problem["detail"], "Synthetic data cannot be exported as a health record.",
            "{path}"
        );
    }
    f.db.cleanup().await;
}

#[tokio::test]
async fn another_orgs_or_unknown_session_is_404() {
    let Some(f) = Fixture::new(false).await else {
        return;
    };
    ingest_ok(&f.router, &f.device, &golden()).await;
    let other_org = seed_quiet_org(&f.db.pool).await;
    let (_, other) = seed_user(&f.db.pool, Some(other_org), "owner").await;
    let own = f.user("owner").await;

    for (user, id) in [(&other, SESSION), (&own, "SESS-UNKNOWN")] {
        for path in ["report.pdf", "fhir"] {
            let (status, _, problem) = send(
                f.router.clone(),
                user_get(user, &format!("/v1/sessions/{id}/{path}")),
            )
            .await;
            assert_eq!(status, StatusCode::NOT_FOUND, "{id}/{path}");
            assert_eq!(problem_slug(&problem), "not-found");
        }
    }
    f.db.cleanup().await;
}

#[tokio::test]
async fn unknown_report_source_is_400() {
    let Some(f) = Fixture::new(false).await else {
        return;
    };
    ingest_ok(&f.router, &f.device, &golden()).await;
    let user = f.user("viewer").await;
    let (status, _, problem) = send(
        f.router.clone(),
        user_get(
            &user,
            &format!("/v1/sessions/{SESSION}/report.pdf?source=cloud"),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(problem_slug(&problem), "validation-failed");
    assert_eq!(problem["invalid_params"][0]["name"], "source");
    f.db.cleanup().await;
}
