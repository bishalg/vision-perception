//! owner: WP-R.3
//!
//! API-3 and CON-10, CON-15, CON-16, CON-17, CON-19, CON-20: `/v1` DTO key names match
//! what the iOS client and the console read, and every example published in `api-v1.json`
//! validates against its own definition (the console contract test of WP-C.8 reads the
//! same examples).

use asd_contract::api_v1::{
    ClipList, ClipScore, MetricRow, RegisterDeviceRequest, RegisterDeviceResponse,
    RegistrationStatus, Role, SessionAttentionFlag, SessionPage, SessionSummary, TokenPair, User,
};
use asd_contract::problem::{self, Problem};
use asd_contract::schema;
use asd_contract::session_v1_6::{SessionIngestResponse, SessionIngestStatus};
use asd_core::types::{CaptureSource, ExpectationStatus, NormativeDirection};
use serde_json::{json, Value};

fn keys(v: &Value) -> Vec<String> {
    let mut k: Vec<String> = v.as_object().unwrap().keys().cloned().collect();
    k.sort();
    k
}

#[test]
fn register_response_matches_the_ios_decoder() {
    // CON-10: `{status, device_id, device_token, org_id, config_bundle_version: string|null}`.
    let r = RegisterDeviceResponse {
        status: RegistrationStatus::Registered,
        device_id: "dev-1".into(),
        device_token: "dvc_abc".into(),
        org_id: "org-1".into(),
        config_bundle_version: None,
    };
    let v = serde_json::to_value(&r).unwrap();
    assert_eq!(
        v,
        json!({
            "status": "registered",
            "device_id": "dev-1",
            "device_token": "dvc_abc",
            "org_id": "org-1",
            "config_bundle_version": null
        })
    );
    let rotated = RegisterDeviceResponse {
        status: RegistrationStatus::Rotated,
        ..r
    };
    assert_eq!(serde_json::to_value(rotated).unwrap()["status"], "rotated");
}

#[test]
fn register_request_matches_what_ios_sends() {
    let body = json!({"enrol_code": "ABCD-1234", "platform": "iOS", "app_version": "1.0.0"});
    let r: RegisterDeviceRequest = serde_json::from_value(body.clone()).unwrap();
    assert_eq!(
        r.device_id, None,
        "device_id is optional until WP-A.11 (CON-11)"
    );
    assert_eq!(serde_json::to_value(&r).unwrap(), body);
    let with_id: RegisterDeviceRequest = serde_json::from_value(
        json!({"enrol_code": "X", "platform": "iOS", "app_version": "1", "device_id": "dev-1"}),
    )
    .unwrap();
    assert_eq!(with_id.device_id.as_deref(), Some("dev-1"));
}

#[test]
fn ingest_response_uses_the_keys_ios_decodes() {
    let r = SessionIngestResponse {
        session_id: "S1".into(),
        status: SessionIngestStatus::Accepted,
        duplicate: false,
        frames_upload_url: Some("/v1/sessions/S1/blobs/frames".into()),
        report_upload_url: None,
    };
    let v = serde_json::to_value(&r).unwrap();
    assert_eq!(
        keys(&v),
        [
            "duplicate",
            "frames_upload_url",
            "report_upload_url",
            "sessionId",
            "status"
        ]
    );
    assert_eq!(v["status"], "accepted");
    assert_eq!(v["report_upload_url"], Value::Null);
}

#[test]
fn token_pair_matches_the_console() {
    let t = TokenPair {
        access_token: "a".into(),
        refresh_token: "r".into(),
        expires_in: 900,
        user: User {
            id: "u1".into(),
            email: "a@example.org".into(),
            role: Role::Clinician,
            org_id: Some("o1".into()),
        },
    };
    let v = serde_json::to_value(&t).unwrap();
    assert_eq!(
        keys(&v),
        ["access_token", "expires_in", "refresh_token", "user"]
    );
    assert_eq!(keys(&v["user"]), ["email", "id", "org_id", "role"]);
    assert_eq!(v["user"]["role"], "clinician");
}

#[test]
fn every_console_role_exists() {
    for (role, wire) in [
        (Role::Owner, "owner"),
        (Role::Admin, "admin"),
        (Role::Clinician, "clinician"),
        (Role::Researcher, "researcher"),
        (Role::Reviewer, "reviewer"),
        (Role::Viewer, "viewer"),
        (Role::Superadmin, "superadmin"),
    ] {
        assert_eq!(serde_json::to_value(role).unwrap(), wire);
    }
}

#[test]
fn session_summary_widens_battery_capture_source_and_flag() {
    let s: SessionSummary = serde_json::from_value(json!({
        "id": "S1",
        "participant_id": "7f1c1c1e-0000-4000-8000-000000000001",
        "participant_token": "child-S1",
        "started_at": "2026-10-26T07:33:20.000Z",
        "duration_sec": 32.5,
        "battery": "express_30s",
        "capture_source": "synthetic",
        "quality_verdict": "optimal",
        "attention_flag": "unreliable"
    }))
    .unwrap();
    assert_eq!(s.battery.as_deref(), Some("express_30s"), "CON-16");
    assert_eq!(s.capture_source, Some(CaptureSource::Synthetic));
    assert_eq!(s.attention_flag, SessionAttentionFlag::Unreliable, "CON-15");
    assert_eq!(s.participant_token, "child-S1", "CON-20");
}

#[test]
fn clip_rows_carry_per_metric_rows() {
    // CON-19: deviation = recorded - expected; flat columns from the worst row.
    let row = MetricRow {
        segment_id: "faces".into(),
        metric: "eye_dwell_ratio".into(),
        target_aoi: None,
        direction: NormativeDirection::HigherBetter,
        expected: Some(0.4),
        acceptable_low: Some(0.3),
        acceptable_high: None,
        recorded: 0.33,
        deviation: Some(0.33 - 0.4),
        status: ExpectationStatus::Monitor,
        score: 0.7,
        imputed: false,
        source: "literature".into(),
    };
    let v = serde_json::to_value(&row).unwrap();
    assert_eq!(
        keys(&v),
        [
            "acceptable_high",
            "acceptable_low",
            "deviation",
            "direction",
            "expected",
            "imputed",
            "metric",
            "recorded",
            "score",
            "segment_id",
            "source",
            "status",
            "target_aoi"
        ]
    );
    let clip: ClipScore = serde_json::from_value(json!({
        "clip_id": "stim_05_geopref_split_10s",
        "order": 1,
        "status": "monitor",
        "rows": [v]
    }))
    .unwrap();
    assert_eq!(clip.rows.len(), 1);
    let list = serde_json::to_value(ClipList { items: vec![clip] }).unwrap();
    assert!(list["items"][0]["rows"][0]["deviation"].is_number());
}

#[test]
fn list_envelope_has_items_next_cursor_total() {
    let page = SessionPage {
        items: vec![],
        next_cursor: None,
        total: None,
    };
    let v = serde_json::to_value(page).unwrap();
    assert_eq!(keys(&v), ["items", "next_cursor", "total"]);
}

#[test]
fn problems_are_rfc7807_with_the_curv_type_prefix() {
    let p = Problem::new(problem::VALIDATION_FAILED, 400, "Validation failed")
        .with_detail("sessionId is required")
        .with_invalid_param("sessionId", "missing");
    let v = serde_json::to_value(&p).unwrap();
    assert_eq!(
        v["type"],
        "https://schemas.curv.org/asd/problems/validation-failed"
    );
    assert_eq!(v["status"], 400);
    assert_eq!(v["invalid_params"][0]["name"], "sessionId");
    assert_eq!(problem::CONTENT_TYPE, "application/problem+json");
    for slug in [
        problem::VALIDATION_FAILED,
        problem::UNSUPPORTED_SCHEMA_VERSION,
        problem::IDEMPOTENCY_KEY_REUSED,
        problem::ENROL_CODE_EXHAUSTED,
        problem::CAPTURE_SOURCE_REJECTED,
    ] {
        assert!(problem::type_uri(slug).ends_with(slug));
    }
}

#[test]
fn api_schema_declares_every_dto() {
    let api = schema::api_v1();
    assert_eq!(api["$id"], "https://schemas.curv.org/asd/api-v1.json");
    let defs = api["$defs"].as_object().unwrap();
    for name in [
        "RegisterDeviceRequest",
        "RegisterDeviceResponse",
        "SessionIngestRequest",
        "SessionIngestResponse",
        "LoginRequest",
        "RefreshRequest",
        "TokenPair",
        "User",
        "Org",
        "Metrics",
        "SessionSummary",
        "SessionDetail",
        "SessionPage",
        "ClipScore",
        "ClipList",
        "MetricRow",
        "Problem",
        "ConfigBundle",
        "ScoringProfile",
        "EnrolCodeRequest",
        "EnrolCodeResponse",
        "Device",
        "ReviewItem",
        "ConfigDocument",
        "AuditEntry",
        "Health",
        "Ready",
    ] {
        assert!(defs.contains_key(name), "api-v1.json lacks $defs/{name}");
    }
    assert_eq!(
        defs["SessionAttentionFlag"]["enum"],
        json!(["green", "yellow", "red", "unreliable"])
    );
    assert_eq!(
        api["$defs"]["SessionSummary"]["properties"]["battery"]["anyOf"][0]["type"], "string",
        "CON-16: battery is the catalog protocol id"
    );
}

#[test]
fn every_published_example_validates_against_its_definition() {
    let api = schema::api_v1();
    let defs = api["$defs"].as_object().unwrap();
    let mut checked = 0;
    for (name, def) in defs {
        let Some(examples) = def.get("examples").and_then(Value::as_array) else {
            continue;
        };
        let mut root = api.clone();
        root.as_object_mut().unwrap().remove("$id");
        root["$ref"] = json!(format!("#/$defs/{name}"));
        for ex in examples {
            schema::validate(&root, ex)
                .unwrap_or_else(|e| panic!("example of {name} invalid: {e:?}"));
            checked += 1;
        }
    }
    assert!(checked >= 10, "only {checked} examples published");
}

// ---- bootstrap/contract fixes (post WP-B.2/B.3) ----------------------------------------

#[test]
fn superadmin_user_has_a_null_org_id() {
    let u = User {
        id: "u1".into(),
        email: "root@example.org".into(),
        role: Role::Superadmin,
        org_id: None,
    };
    let v = serde_json::to_value(&u).unwrap();
    assert_eq!(keys(&v), ["email", "id", "org_id", "role"]);
    assert!(v["org_id"].is_null(), "written as null, not omitted: {v}");
    let back: User = serde_json::from_value(v).unwrap();
    assert_eq!(back.org_id, None);

    let api = schema::api_v1();
    let org_id = &api["$defs"]["User"]["properties"]["org_id"];
    assert_eq!(
        org_id["anyOf"],
        json!([{"type": "string"}, {"type": "null"}]),
        "{org_id}"
    );
}

#[test]
fn user_status_includes_invited() {
    use asd_contract::api_v1::UserStatus;
    for (status, wire) in [
        (UserStatus::Active, "active"),
        (UserStatus::Disabled, "disabled"),
        (UserStatus::Invited, "invited"),
    ] {
        assert_eq!(serde_json::to_value(status).unwrap(), wire);
    }
    assert_eq!(
        schema::api_v1()["$defs"]["UserStatus"]["enum"],
        json!(["active", "disabled", "invited"])
    );
}

#[test]
fn participant_withdrawn_is_its_own_problem_slug() {
    assert_eq!(problem::PARTICIPANT_WITHDRAWN, "participant-withdrawn");
    assert_ne!(problem::PARTICIPANT_WITHDRAWN, problem::CONFLICT);
    assert!(problem::SLUGS.contains(&problem::PARTICIPANT_WITHDRAWN));
}

#[test]
fn api_schema_publishes_the_contract_problem_slugs() {
    let api = schema::api_v1();
    let published = &api["$defs"]["ProblemSlug"]["enum"];
    assert_eq!(published, &json!(problem::SLUGS));
    for slug in [
        problem::VALIDATION_FAILED,
        problem::UNSUPPORTED_SCHEMA_VERSION,
        problem::IDEMPOTENCY_KEY_REUSED,
        problem::ENROL_CODE_EXHAUSTED,
        problem::CAPTURE_SOURCE_REJECTED,
        problem::UNAUTHORIZED,
        problem::FORBIDDEN,
        problem::NOT_FOUND,
        problem::RATE_LIMITED,
        problem::CONFLICT,
        problem::PARTICIPANT_WITHDRAWN,
        problem::INTERNAL,
    ] {
        assert!(problem::SLUGS.contains(&slug), "{slug} missing from SLUGS");
    }
    let mut unique = problem::SLUGS.to_vec();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(unique.len(), problem::SLUGS.len(), "no duplicates");
}
