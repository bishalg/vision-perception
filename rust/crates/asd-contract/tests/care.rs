//! owner: WP-B.6
//!
//! API-1/API-3: the care bodies keep their snake_case keys, default sensibly, reject
//! unknown kinds, and are published in `api-v1.json`.

use asd_contract::care::{CareEventKind, CareEventsRequest, CareEventsResponse};
use asd_contract::schema;
use serde_json::json;

#[test]
fn request_defaults_payload_and_optionals() {
    let r: CareEventsRequest = serde_json::from_value(json!({
        "participant_token": "kid",
        "events": [{"kind": "milestone"}]
    }))
    .unwrap();
    assert_eq!(r.events[0].kind, CareEventKind::Milestone);
    assert_eq!(r.events[0].payload, json!({}));
    assert!(r.events[0].occurred_at.is_none() && r.events[0].idempotency_key.is_none());
}

#[test]
fn unknown_kind_is_rejected() {
    let r = serde_json::from_value::<CareEventsRequest>(json!({
        "participant_token": "kid",
        "events": [{"kind": "sleep"}]
    }));
    assert!(r.is_err());
}

#[test]
fn response_keys() {
    let v = serde_json::to_value(CareEventsResponse {
        participant_token: "kid".into(),
        accepted: 2,
        duplicates: 1,
        duplicate: false,
    })
    .unwrap();
    assert_eq!(
        v,
        json!({"participant_token": "kid", "accepted": 2, "duplicates": 1, "duplicate": false})
    );
}

#[test]
fn care_types_are_in_the_api_schema() {
    let defs = &schema::api_v1()["$defs"];
    for name in [
        "CareEventsRequest",
        "CareEventsResponse",
        "CareSummary",
        "ErasureRequestResponse",
        "CareEventKind",
    ] {
        assert!(defs.get(name).is_some(), "{name} missing from api-v1.json");
    }
}
