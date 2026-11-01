//! owner: WP-A.5
//!
//! The config bundle entry points as Swift calls them, on the signed fixtures the iOS tests
//! use (`apps/ios_asd_gaze_app/Tests/Fixtures/config-bundle/`, see `asd-sync/tests/config_bundle.rs`).

use std::path::PathBuf;

use asd_ffi::*;
use serde_json::Value;

const NOW: i64 = 1_793_577_600;

fn fixture(name: &str) -> String {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../apps/ios_asd_gaze_app/Tests/Fixtures/config-bundle");
    std::fs::read_to_string(dir.join(name)).unwrap()
}

fn ring() -> Vec<AsdTrustedKey> {
    let keys: Value = serde_json::from_str(&fixture("test-keys.json")).unwrap();
    vec![AsdTrustedKey {
        key_id: keys["trusted"]["keyId"].as_str().unwrap().into(),
        public_key_b64: keys["trusted"]["publicKey"].as_str().unwrap().into(),
    }]
}

fn reason(v: AsdBundleVerdict) -> Option<AsdBundleRejection> {
    match v {
        AsdBundleVerdict::Accepted { .. } => None,
        AsdBundleVerdict::Rejected { reason, .. } => Some(reason),
    }
}

#[test]
fn accepted_bundle_hands_back_decodable_documents() {
    let AsdBundleVerdict::Accepted { bundle } =
        config_accept_bundle(fixture("bundle-valid.json"), ring(), None, NOW, None)
    else {
        panic!("valid fixture refused");
    };
    assert_eq!(bundle.bundle_version, "2026.11.01-2");
    let profile: Value = serde_json::from_str(&bundle.scoring_profile_json).unwrap();
    assert_eq!(profile["flag"]["greenFloor"], Value::from(0.8));
    assert_eq!(profile["lowConfidenceValidRatio"], Value::from(0.7));
    let catalog: Value = serde_json::from_str(&bundle.catalog_json).unwrap();
    assert!(catalog["clips"].as_array().is_some_and(|c| !c.is_empty()));
    assert_eq!(bundle.feature_flags.get("norm_collection"), Some(&false));
}

#[test]
fn refusals_carry_their_reason() {
    let r = |name: &str, newest: Option<&str>, max_age: Option<u64>| {
        reason(config_accept_bundle(
            fixture(name),
            ring(),
            newest.map(str::to_string),
            NOW,
            max_age,
        ))
    };
    assert_eq!(
        r("bundle-unknown-key.json", None, None),
        Some(AsdBundleRejection::UnknownKey)
    );
    assert_eq!(
        r("bundle-wrong-key.json", None, None),
        Some(AsdBundleRejection::BadSignature)
    );
    assert_eq!(
        r("bundle-older.json", Some("2026.11.01-2"), None),
        Some(AsdBundleRejection::Rollback)
    );
    assert_eq!(
        r("bundle-older.json", None, Some(86_400)),
        Some(AsdBundleRejection::Expired)
    );
    assert_eq!(
        reason(config_accept_bundle("{".into(), ring(), None, NOW, None)),
        Some(AsdBundleRejection::Malformed)
    );
    assert_eq!(
        reason(config_accept_bundle(
            fixture("bundle-valid.json"),
            vec![],
            None,
            NOW,
            None
        )),
        Some(AsdBundleRejection::NoTrustedKeys)
    );
}

#[test]
fn profile_threshold_reaches_scoring() {
    let set = serde_json::json!([{
        "stimulusId": "clip", "clipFrameCount": 100, "validFrameCount": 65,
        "validSampleRatio": 0.65, "observedDurationSec": 10.0,
        "aoiDwellRatios": {}, "metrics": {}
    }]);
    let baseline = serde_json::json!({
        "schemaVersion": "1", "status": "provisional", "ageBrackets": [], "videos": {}
    });
    let mut profile: Value = serde_json::from_str(&default_scoring_profile_json()).unwrap();
    let low = |profile: &Value| -> bool {
        let out = score_session_with_profile_json(
            set.to_string(),
            baseline.to_string(),
            24,
            "[]".into(),
            String::new(),
            profile.to_string(),
        )
        .unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        v["videos"][0]["lowConfidence"].as_bool().unwrap()
    };
    assert!(!low(&profile));
    profile["lowConfidenceValidRatio"] = Value::from(0.7);
    assert!(low(&profile));
    assert!(matches!(
        score_session_with_profile_json(
            "[]".into(),
            baseline.to_string(),
            1,
            "[]".into(),
            String::new(),
            "{}".into()
        ),
        Err(AsdError::InvalidJson { .. })
    ));
}

#[test]
fn bundle_request_and_response() {
    let client = AsdClientInfo {
        product: "EyeGaze-iOS".into(),
        app_version: "1.0.0".into(),
        build: "3".into(),
        os_name: "iOS".into(),
        os_version: "18.0".into(),
    };
    let r = config_build_bundle_request("tok".into(), Some("2026.11.01-2".into()), client.clone())
        .unwrap();
    assert_eq!(r.method, "GET");
    assert_eq!(r.path, "v1/config/bundle");
    assert!(r
        .headers
        .iter()
        .any(|h| h.name == "If-None-Match" && h.value == "\"2026.11.01-2\""));
    assert!(matches!(
        config_build_bundle_request(" ".into(), None, client),
        Err(AsdError::NotRegistered { .. })
    ));
    assert_eq!(
        config_classify_bundle_response(304, vec![]),
        AsdConfigBundleOutcome::NotModified
    );
    assert_eq!(
        config_classify_bundle_response(404, vec![]),
        AsdConfigBundleOutcome::NoBundle
    );
}
