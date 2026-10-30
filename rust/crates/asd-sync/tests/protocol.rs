//! owner: WP-R.3
//!
//! CON-7: ports of the iOS sync client cases (`SessionSyncServiceTests`,
//! `SessionIngestPayloadTests`, `DeviceRegistrationServiceTests`,
//! `ResearchStudyConfigurationTests`): headers, body bytes, 401/403 -> re-authenticate,
//! 409 -> duplicate, Retry-After cap, backoff schedule, URL rules.

use std::path::PathBuf;

use asd_contract::session_v1_6::SessionIngestRequest;
use asd_core::types::CaptureSource;
use asd_sync::{
    backoff_schedule, build_register_request, build_session_request, build_session_request_json,
    classify_register_response, classify_response, endpoint_url, next_retry_delay,
    retry_after_seconds, validate_ingest_url, validate_upload_url, ClientInfo, RegisterOutcome,
    SyncError, SyncOutcome, UrlError, BACKOFF_DELAYS_SECS, MAX_ATTEMPTS, REGISTER_PATH,
    SESSIONS_PATH,
};
use serde_json::{json, Value};

fn fixture() -> SessionIngestRequest {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../apps/ios_asd_gaze_app/Tests/Fixtures/expected-session-v1.6.json");
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn client() -> ClientInfo {
    ClientInfo::ios("1.0.0", "3", "18.0")
}

fn header<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(name))
        .map(|(_, v)| v.as_str())
}

fn h(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

// ---- request building ------------------------------------------------------------------

#[test]
fn session_request_sends_the_contract_headers() {
    // testSessionUploadSendsContractHeadersAndPayload
    let mut payload = fixture();
    payload.session_id = "SESS-HDR-1".into();
    let req = build_session_request(&payload, "tok-headers", &client()).unwrap();
    assert_eq!(req.method, "POST");
    assert_eq!(req.path, SESSIONS_PATH);
    assert_eq!(SESSIONS_PATH, "v1/sessions");
    assert_eq!(req.timeout_secs, 20);
    let hd = &req.headers;
    assert_eq!(header(hd, "Authorization"), Some("Bearer tok-headers"));
    assert_eq!(header(hd, "X-Idempotency-Key"), Some("SESS-HDR-1"));
    assert_eq!(header(hd, "X-Schema-Version"), Some("1.6"));
    assert_eq!(
        header(hd, "User-Agent"),
        Some("EyeGaze-iOS/1.0.0 (3; iOS 18.0)")
    );
    assert_eq!(header(hd, "Content-Type"), Some("application/json"));
    assert_eq!(header(hd, "Accept"), Some("application/json"));
    assert_eq!(hd.len(), 6);
}

#[test]
fn body_is_the_canonical_payload_with_sorted_keys_and_iso_dates() {
    let payload = fixture();
    let req = build_session_request(&payload, "t", &client()).unwrap();
    let body: Value = serde_json::from_slice(&req.body).unwrap();
    assert_eq!(body["sessionId"], "SESS-GOLDEN-0001");
    assert_eq!(body["rawPayload"]["startedAt"], "2026-10-26T07:33:20.000Z");
    assert_eq!(body["attentionFlag"], "yellow");
    assert_eq!(body["battery"], Value::Null, "explicit null kept");
    // Bytes are the RFC 8785 form: sorted keys, Swift-like numbers (`38`, not `38.0`).
    assert_eq!(
        req.body,
        asd_contract::signing::canonical_bytes(&payload).unwrap()
    );
    let text = String::from_utf8(req.body.clone()).unwrap();
    assert!(text.starts_with("{\"attentionFlag\":"), "{text}");
    assert!(text.contains("\"calibratedDistanceCm\":38,"));
}

#[test]
fn idempotency_key_is_stable_across_retries() {
    let payload = fixture();
    let a = build_session_request(&payload, "t1", &client()).unwrap();
    let b = build_session_request(&payload, "t2", &client()).unwrap();
    assert_eq!(
        header(&a.headers, "X-Idempotency-Key"),
        header(&b.headers, "X-Idempotency-Key")
    );
    assert_eq!(
        a.body, b.body,
        "same bytes, so the server's body hash matches"
    );
}

#[test]
fn simulated_and_legacy_records_are_refused() {
    let mut p = fixture();
    p.capture_source = CaptureSource::Simulated;
    assert_eq!(
        build_session_request(&p, "t", &client()).unwrap_err(),
        SyncError::SimulatedCapture
    );
    let mut p = fixture();
    p.raw_payload.legacy_provenance = Some(true);
    assert_eq!(
        build_session_request(&p, "t", &client()).unwrap_err(),
        SyncError::LegacyProvenance
    );
}

#[test]
fn schema_version_must_be_supported() {
    let mut p = fixture();
    p.schema_version = "1.7".into();
    assert_eq!(
        build_session_request(&p, "t", &client()).unwrap_err(),
        SyncError::UnsupportedSchemaVersion("1.7".into())
    );
}

#[test]
fn missing_token_is_not_registered() {
    assert_eq!(
        build_session_request(&fixture(), "  ", &client()).unwrap_err(),
        SyncError::NotRegistered
    );
}

#[test]
fn dates_must_be_rfc3339_utc_with_milliseconds() {
    for bad in [
        "2026-10-26T07:33:20Z",
        "2026-10-26T07:33:20.000+01:00",
        "2026-10-26 07:33:20.000Z",
        "26/10/2026",
        "2026-13-26T07:33:20.000Z",
    ] {
        let mut p = fixture();
        p.raw_payload.started_at = bad.into();
        assert!(
            matches!(
                build_session_request(&p, "t", &client()),
                Err(SyncError::InvalidDate { .. })
            ),
            "{bad}"
        );
    }
}

#[test]
fn json_entry_point_validates_the_payload() {
    let bytes = serde_json::to_vec(&fixture()).unwrap();
    let req = build_session_request_json(&bytes, "t", &client()).unwrap();
    assert_eq!(req.path, SESSIONS_PATH);
    let mut v: Value = serde_json::from_slice(&bytes).unwrap();
    v.as_object_mut().unwrap().remove("sessionId");
    assert!(matches!(
        build_session_request_json(&serde_json::to_vec(&v).unwrap(), "t", &client()),
        Err(SyncError::InvalidPayload(_))
    ));
    assert!(build_session_request_json(b"{", "t", &client()).is_err());
}

#[test]
fn register_request_posts_the_enrol_code() {
    // testRegisterPostsEnrolCodeToDevicesRegister
    let req = build_register_request(" ABCD-1234 \n", None, "idem-1", &client()).unwrap();
    assert_eq!(req.method, "POST");
    assert_eq!(req.path, REGISTER_PATH);
    assert_eq!(REGISTER_PATH, "v1/devices/register");
    assert_eq!(req.timeout_secs, 15);
    assert_eq!(header(&req.headers, "X-Idempotency-Key"), Some("idem-1"));
    assert_eq!(header(&req.headers, "Authorization"), None);
    let body: Value = serde_json::from_slice(&req.body).unwrap();
    assert_eq!(
        body,
        json!({"app_version": "1.0.0", "enrol_code": "ABCD-1234", "platform": "iOS"})
    );
    // CON-11: re-registration names the device so the server rotates its token.
    let req = build_register_request("ABCD-1234", Some("dev-1"), "idem-2", &client()).unwrap();
    let body: Value = serde_json::from_slice(&req.body).unwrap();
    assert_eq!(body["device_id"], "dev-1");
}

#[test]
fn blank_enrol_code_is_rejected_without_a_request() {
    assert_eq!(
        build_register_request("  \n", None, "i", &client()).unwrap_err(),
        SyncError::EmptyEnrolCode
    );
}

#[test]
fn register_response_classification() {
    let ok = json!({"status": "registered", "device_id": "d", "device_token": "dvc_x",
                    "org_id": "o", "config_bundle_version": null});
    match classify_register_response(201, &serde_json::to_vec(&ok).unwrap()) {
        RegisterOutcome::Registered(r) => {
            assert_eq!(r.device_token, "dvc_x");
            assert_eq!(r.config_bundle_version, None);
        }
        other => panic!("{other:?}"),
    }
    // testResponseWithoutTokenIsInvalid
    let no_token = json!({"status": "registered", "device_id": "d", "device_token": "",
                          "org_id": "o", "config_bundle_version": null});
    assert_eq!(
        classify_register_response(200, &serde_json::to_vec(&no_token).unwrap()),
        RegisterOutcome::Invalid
    );
    assert_eq!(
        classify_register_response(200, b"nope"),
        RegisterOutcome::Invalid
    );
    // CON-11: an exhausted code without device_id is 410.
    assert_eq!(
        classify_register_response(410, b"{}"),
        RegisterOutcome::Rejected { status: 410 }
    );
}

// ---- URL rules (F7) --------------------------------------------------------------------

#[test]
fn endpoint_url_keeps_the_base_path_prefix() {
    // testBaseURLPathPrefixIsKept
    assert_eq!(
        endpoint_url("https://api.example.org", SESSIONS_PATH).unwrap(),
        "https://api.example.org/v1/sessions"
    );
    assert_eq!(
        endpoint_url("https://api.example.org/base", REGISTER_PATH).unwrap(),
        "https://api.example.org/base/v1/devices/register"
    );
    assert_eq!(
        endpoint_url("https://api.example.org/base/", SESSIONS_PATH).unwrap(),
        "https://api.example.org/base/v1/sessions"
    );
    assert!(endpoint_url("http://api.example.org", SESSIONS_PATH).is_err());
    assert!(endpoint_url("https://127.0.0.1", SESSIONS_PATH).is_err());
}

#[test]
fn https_with_port_and_uppercase_scheme_are_accepted() {
    validate_ingest_url("https://ingest.example.org:8443/ingest").unwrap();
    validate_ingest_url("HTTPS://ingest.example.org/ingest").unwrap();
    validate_ingest_url("https://ingest.example.org/x").unwrap();
}

#[test]
fn non_https_schemes_are_rejected() {
    assert_eq!(
        validate_ingest_url("http://ingest.example.org/api/v1/research/ingest"),
        Err(UrlError::InsecureScheme("http".into()))
    );
    for raw in [
        "http://127.0.0.1:8089/api/v1/research/ingest",
        "ftp://ingest.example.org/upload",
        "ws://ingest.example.org/socket",
        "wss://ingest.example.org/socket",
        "file:///tmp/ingest",
        "data:application/json,{}",
        "ingest.example.org/api",
        "//ingest.example.org/api",
        "httpss://ingest.example.org/api",
        "not a url",
        "   ",
    ] {
        assert!(validate_ingest_url(raw).is_err(), "{raw}");
    }
}

#[test]
fn ip_literal_hosts_are_rejected_even_over_https() {
    for raw in [
        "https://127.0.0.1/ingest",
        "https://127.0.0.1:8089/api/v1/research/ingest",
        "https://10.0.0.5/ingest",
        "https://192.168.1.20:443/ingest",
        "https://[::1]/ingest",
        "https://[2001:db8::1]:8443/ingest",
        "https://127.1/ingest",
        "https://2130706433/ingest",
        "https://0x7f.0.0.1/ingest",
    ] {
        assert!(
            matches!(validate_ingest_url(raw), Err(UrlError::IpLiteralHost(_))),
            "{raw}: {:?}",
            validate_ingest_url(raw)
        );
    }
    assert_eq!(
        validate_ingest_url("https://127.0.0.1/ingest"),
        Err(UrlError::IpLiteralHost("127.0.0.1".into()))
    );
}

#[test]
fn localhost_credentials_and_missing_host_are_rejected() {
    for raw in [
        "https://localhost/ingest",
        "https://LOCALHOST:8443/ingest",
        "https://api.localhost/ingest",
    ] {
        assert!(
            matches!(validate_ingest_url(raw), Err(UrlError::LoopbackHost(_))),
            "{raw}"
        );
    }
    assert_eq!(
        validate_ingest_url("https://user:secret@ingest.example.org/ingest"),
        Err(UrlError::CredentialsInUrl)
    );
    assert!(validate_ingest_url("https:///ingest").is_err());
}

#[test]
fn presigned_upload_to_a_foreign_host_carries_no_bearer() {
    // testPreSignedFramesUploadToForeignHostCarriesNoBearer
    let raw = "https://blobs.example.net/sessions/org/child/SESS-0001/frames.jsonl.enc?X-Amz-Signature=abc";
    let t = validate_upload_url(raw, "https://api.example.org").unwrap();
    assert_eq!(t.url, raw);
    assert!(!t.attach_bearer);
}

#[test]
fn relative_upload_url_resolves_against_the_api_base_and_carries_the_bearer() {
    // testRelativeUploadURLResolvesAgainstAPIBaseAndCarriesBearer (CON-18 proxy mode)
    let t = validate_upload_url(
        "/v1/sessions/SESS-0001/blobs/frames",
        "https://api.example.org",
    )
    .unwrap();
    assert_eq!(
        t.url,
        "https://api.example.org/v1/sessions/SESS-0001/blobs/frames"
    );
    assert!(t.attach_bearer);
    let t = validate_upload_url(
        "https://API.example.org/v1/sessions/S/blobs/report",
        "https://api.example.org",
    )
    .unwrap();
    assert!(t.attach_bearer, "host comparison is case-insensitive");
}

#[test]
fn insecure_upload_urls_are_never_used() {
    // testInsecureUploadURLIsNeverUsed
    for raw in [
        "http://blobs.example.net/frames",
        "https://10.0.0.1/frames",
        "https://localhost/frames",
        "https://u:p@blobs.example.net/frames",
    ] {
        assert!(
            validate_upload_url(raw, "https://api.example.org").is_err(),
            "{raw}"
        );
    }
}

// ---- response classification -----------------------------------------------------------

#[test]
fn success_is_accepted_and_reads_upload_urls() {
    let body = json!({"sessionId": "S1", "status": "accepted", "duplicate": false,
                      "frames_upload_url": "https://blobs.example.net/f", "report_upload_url": null});
    match classify_response(201, &[], &serde_json::to_vec(&body).unwrap()) {
        SyncOutcome::Accepted(ack) => {
            assert_eq!(ack.session_id.as_deref(), Some("S1"));
            assert_eq!(
                ack.frames_upload_url.as_deref(),
                Some("https://blobs.example.net/f")
            );
            assert_eq!(ack.report_upload_url, None);
            assert_eq!(ack.duplicate, Some(false));
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_sparse_or_garbage_success_body_is_still_accepted() {
    for body in [&b""[..], b"not json", b"{}", b"{\"type\":\"about:blank\"}"] {
        match classify_response(200, &[], body) {
            SyncOutcome::Accepted(ack) => assert_eq!(ack.session_id, None),
            other => panic!("{other:?}"),
        }
    }
}

#[test]
fn replay_with_duplicate_true_is_accepted() {
    // CON-17: same key + same body -> 200 with the original body and duplicate: true.
    let body = br#"{"sessionId":"S1","status":"accepted","duplicate":true}"#;
    match classify_response(200, &[], body) {
        SyncOutcome::Accepted(ack) => assert_eq!(ack.duplicate, Some(true)),
        other => panic!("{other:?}"),
    }
}

#[test]
fn auth_failures_reauthenticate() {
    // testUnauthorizedReRegistersOnceAndRetries
    assert_eq!(
        classify_response(401, &[], b""),
        SyncOutcome::Reauthenticate
    );
    assert_eq!(
        classify_response(403, &[], b""),
        SyncOutcome::Reauthenticate
    );
}

#[test]
fn conflict_is_an_idempotent_replay() {
    // testConflictIsAnIdempotentReplayAndCountsAsSuccess
    assert_eq!(classify_response(409, &[], b"{}"), SyncOutcome::Duplicate);
}

#[test]
fn transient_failures_retry_and_honour_retry_after() {
    // testServerErrorBacksOffAndHonoursRetryAfter
    assert_eq!(
        classify_response(429, &h(&[("Retry-After", "120")]), b""),
        SyncOutcome::RetryAfter(Some(120.0))
    );
    assert_eq!(
        classify_response(503, &h(&[("retry-after", " 5 ")]), b""),
        SyncOutcome::RetryAfter(Some(5.0))
    );
    assert_eq!(
        classify_response(500, &[], b""),
        SyncOutcome::RetryAfter(None)
    );
    assert_eq!(
        classify_response(408, &[], b""),
        SyncOutcome::RetryAfter(None)
    );
    assert_eq!(
        classify_response(502, &[], b""),
        SyncOutcome::RetryAfter(None)
    );
}

#[test]
fn retry_after_is_capped_and_only_delta_seconds_count() {
    assert_eq!(
        retry_after_seconds(&h(&[("Retry-After", "999999")])),
        Some(3600.0)
    );
    assert_eq!(
        retry_after_seconds(&h(&[("Retry-After", "1.5")])),
        Some(1.5)
    );
    for bad in [
        "Wed, 21 Oct 2026 07:28:00 GMT",
        "-5",
        "soon",
        "",
        "inf",
        "NaN",
    ] {
        assert_eq!(
            retry_after_seconds(&h(&[("Retry-After", bad)])),
            None,
            "{bad}"
        );
    }
    assert_eq!(retry_after_seconds(&[]), None);
}

#[test]
fn permanent_client_errors_are_fatal() {
    // testUnprocessablePayloadIsQuarantined
    for status in [400u16, 404, 410, 413, 415, 422] {
        assert_eq!(
            classify_response(status, &[], b""),
            SyncOutcome::Fatal { status }
        );
    }
    assert_eq!(
        classify_response(302, &[], b""),
        SyncOutcome::Fatal { status: 302 }
    );
}

// ---- backoff ---------------------------------------------------------------------------

#[test]
fn backoff_schedule_matches_the_research_queue() {
    // ResearchSyncRetryPolicy: 1, 2, 4, 8, 60; dead letter on the sixth failure.
    assert_eq!(BACKOFF_DELAYS_SECS, [1, 2, 4, 8, 60]);
    assert_eq!(MAX_ATTEMPTS, 6);
    let got: Vec<u64> = (0..=7).map(backoff_schedule).collect();
    assert_eq!(got, [1, 1, 2, 4, 8, 60, 60, 60]);
}

#[test]
fn next_delay_is_the_larger_of_backoff_and_retry_after() {
    assert_eq!(next_retry_delay(1, Some(120.0)), 120.0);
    assert_eq!(next_retry_delay(1, None), 1.0);
    assert_eq!(next_retry_delay(5, Some(3.0)), 60.0);
}
