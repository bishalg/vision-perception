//! owner: WP-A.5
//!
//! Device acceptance of config bundles (CON-6, 02 ADR-B6): signature against the app key
//! ring, key rotation by `key_id`, rollback and age policy, request and response rules.
//!
//! The signed fixtures under `apps/ios_asd_gaze_app/Tests/Fixtures/config-bundle/` are the
//! inputs of the iOS tests too, so both sides decide on the same bytes. They are signed with
//! fixed test seeds (never a production key). Regenerate with
//! `ASD_REGEN_BUNDLE_FIXTURES=1 cargo test -p asd-sync --test config_bundle -- regenerate` (the catalog and
//! baselines come from `ASD_BUNDLE_FIXTURE_RES` or the kit's bundled resources).

use std::cmp::Ordering;
use std::path::PathBuf;

use asd_contract::config::{ConfigBundle, ScoringProfile};
use asd_contract::signing::{self, SigningKey};
use asd_sync::{
    accept_bundle, build_config_bundle_request, classify_config_bundle_response,
    compare_bundle_versions, BundlePolicy, BundleRejection, ClientInfo, ConfigBundleOutcome,
    SyncError, TrustedKey, CONFIG_BUNDLE_PATH,
};
use serde_json::Value;

/// Seed of the trusted test key ("key A"), also embedded in the iOS tests.
const SEED_A: [u8; 32] = [0xA1; 32];
/// Seed of an untrusted key ("key B").
const SEED_B: [u8; 32] = [0xB2; 32];

const VALID: &str = "bundle-valid.json";
const OLDER: &str = "bundle-older.json";
const UNKNOWN_KEY: &str = "bundle-unknown-key.json";
const WRONG_KEY: &str = "bundle-wrong-key.json";
const TEST_KEYS: &str = "test-keys.json";

/// 2026-11-02T00:00:00Z: one day after the valid fixture was signed.
const NOW: i64 = 1_793_577_600;

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../apps/ios_asd_gaze_app/Tests/Fixtures/config-bundle")
}

fn key(seed: [u8; 32]) -> SigningKey {
    signing::signing_key_from_seed(seed)
}

fn trusted(seed: [u8; 32]) -> TrustedKey {
    let k = key(seed).verifying_key();
    TrustedKey {
        key_id: signing::key_id(&k),
        public_key_b64: signing::encode_public_key(&k),
    }
}

fn ring() -> Vec<TrustedKey> {
    vec![trusted(SEED_A)]
}

fn read(name: &str) -> String {
    std::fs::read_to_string(fixtures_dir().join(name))
        .unwrap_or_else(|e| panic!("{name}: {e} (regenerate, see the module docs)"))
}

fn policy(newest: Option<&str>, max_age_secs: Option<u64>) -> BundlePolicy {
    BundlePolicy {
        newest_accepted_version: newest.map(str::to_string),
        now_unix_secs: NOW,
        max_age_secs,
    }
}

/// The delivered profile differs from the bundled default, so the apps can see it applied.
fn delivered_profile() -> ScoringProfile {
    let mut p = ScoringProfile::default();
    p.flag.green_floor = 0.80;
    p.low_confidence_valid_ratio = 0.70;
    p
}

fn bundle(version: &str, signed_at: &str, key_id: &str) -> ConfigBundle {
    let res = std::env::var("ASD_BUNDLE_FIXTURE_RES").map_or_else(
        |_| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../../domains/asd_screening/ios/Sources/CurvAsdGazeKit/Resources")
        },
        PathBuf::from,
    );
    let load = |name: &str| -> Value {
        serde_json::from_str(&std::fs::read_to_string(res.join(name)).unwrap()).unwrap()
    };
    ConfigBundle {
        bundle_version: version.into(),
        key_id: key_id.into(),
        catalog: serde_json::from_value(load("stimulus_catalog.json")).unwrap(),
        baselines: serde_json::from_value(load("normative_baselines.json")).unwrap(),
        scoring_profile: delivered_profile(),
        feature_flags: [("norm_collection".to_string(), false)].into(),
        signed_at: signed_at.into(),
        signature: String::new(),
    }
}

fn signed(mut b: ConfigBundle, with: &SigningKey) -> String {
    signing::sign_bundle(&mut b, with).unwrap();
    let mut text = serde_json::to_string_pretty(&b).unwrap();
    text.push('\n');
    text
}

#[test]
fn regenerate_fixtures_when_asked() {
    if std::env::var("ASD_REGEN_BUNDLE_FIXTURES").is_err() {
        return;
    }
    let (a, b) = (key(SEED_A), key(SEED_B));
    let id_a = trusted(SEED_A).key_id;
    let id_b = trusted(SEED_B).key_id;
    let dir = fixtures_dir();
    std::fs::create_dir_all(&dir).unwrap();
    let files = [
        (
            VALID,
            signed(
                bundle("2026.11.01-2", "2026-11-01T00:00:00.000Z", &id_a),
                &a,
            ),
        ),
        (
            OLDER,
            signed(
                bundle("2026.10.30-1", "2026-10-30T00:00:00.000Z", &id_a),
                &a,
            ),
        ),
        // Signed by an untrusted key under its own id.
        (
            UNKNOWN_KEY,
            signed(
                bundle("2026.11.01-2", "2026-11-01T00:00:00.000Z", &id_b),
                &b,
            ),
        ),
        // Signed by an untrusted key that claims the trusted key's id.
        (
            WRONG_KEY,
            signed(
                bundle("2026.11.01-2", "2026-11-01T00:00:00.000Z", &id_a),
                &b,
            ),
        ),
    ];
    for (name, text) in files {
        std::fs::write(dir.join(name), text).unwrap();
    }
    // The public halves for the iOS tests (no seed, no private key).
    let keys = serde_json::json!({
        "trusted": {"keyId": id_a, "publicKey": trusted(SEED_A).public_key_b64},
        "untrusted": {"keyId": id_b, "publicKey": trusted(SEED_B).public_key_b64},
    });
    let mut text = serde_json::to_string_pretty(&keys).unwrap();
    text.push('\n');
    std::fs::write(dir.join(TEST_KEYS), text).unwrap();
}

#[test]
fn test_key_ids_are_stable() {
    // The iOS tests embed these; a change here must be mirrored there.
    let a = trusted(SEED_A);
    assert_eq!(a.public_key_b64.len(), 43);
    let v: Value = serde_json::from_str(&read(VALID)).unwrap();
    assert_eq!(v["key_id"], Value::String(a.key_id.clone()));
    let keys: Value = serde_json::from_str(&read(TEST_KEYS)).unwrap();
    assert_eq!(keys["trusted"]["keyId"], Value::String(a.key_id));
    assert_eq!(
        keys["trusted"]["publicKey"],
        Value::String(a.public_key_b64)
    );
    assert_eq!(
        keys["untrusted"]["publicKey"],
        Value::String(trusted(SEED_B).public_key_b64)
    );
}

#[test]
fn valid_bundle_is_accepted_with_its_documents() {
    let b = accept_bundle(read(VALID).as_bytes(), &ring(), &policy(None, None)).unwrap();
    assert_eq!(b.bundle_version, "2026.11.01-2");
    assert_eq!(b.scoring_profile, delivered_profile());
    assert!(!b.catalog.clips.is_empty());
}

#[test]
fn rotated_ring_accepts_either_key_by_id() {
    let rotated = vec![trusted(SEED_B), trusted(SEED_A)];
    assert!(accept_bundle(read(VALID).as_bytes(), &rotated, &policy(None, None)).is_ok());
    assert!(accept_bundle(read(UNKNOWN_KEY).as_bytes(), &rotated, &policy(None, None)).is_ok());
}

#[test]
fn tampered_bundle_is_refused() {
    let mut v: Value = serde_json::from_str(&read(VALID)).unwrap();
    v["scoring_profile"]["flag"]["greenFloor"] = Value::from(0.1);
    let text = serde_json::to_string(&v).unwrap();
    assert_eq!(
        accept_bundle(text.as_bytes(), &ring(), &policy(None, None)),
        Err(BundleRejection::BadSignature)
    );
    // Whitespace and key order are not part of the signed form.
    let compact =
        serde_json::to_string(&serde_json::from_str::<Value>(&read(VALID)).unwrap()).unwrap();
    assert!(accept_bundle(compact.as_bytes(), &ring(), &policy(None, None)).is_ok());
}

#[test]
fn untrusted_or_impostor_keys_are_refused() {
    assert_eq!(
        accept_bundle(read(UNKNOWN_KEY).as_bytes(), &ring(), &policy(None, None)),
        Err(BundleRejection::UnknownKey(trusted(SEED_B).key_id))
    );
    assert_eq!(
        accept_bundle(read(WRONG_KEY).as_bytes(), &ring(), &policy(None, None)),
        Err(BundleRejection::BadSignature)
    );
}

#[test]
fn unsigned_malformed_and_unconfigured_are_refused() {
    let mut v: Value = serde_json::from_str(&read(VALID)).unwrap();
    v.as_object_mut().unwrap().remove("signature");
    assert_eq!(
        accept_bundle(v.to_string().as_bytes(), &ring(), &policy(None, None)),
        Err(BundleRejection::Unsigned)
    );
    assert!(matches!(
        accept_bundle(b"not json", &ring(), &policy(None, None)),
        Err(BundleRejection::Malformed(_))
    ));
    assert_eq!(
        accept_bundle(read(VALID).as_bytes(), &[], &policy(None, None)),
        Err(BundleRejection::NoTrustedKeys)
    );
    let bad = vec![TrustedKey {
        key_id: "k".into(),
        public_key_b64: "short".into(),
    }];
    assert_eq!(
        accept_bundle(read(VALID).as_bytes(), &bad, &policy(None, None)),
        Err(BundleRejection::MalformedTrustedKey("k".into()))
    );
}

#[test]
fn rollback_below_the_newest_accepted_version_is_refused() {
    let older = read(OLDER);
    assert_eq!(
        accept_bundle(
            older.as_bytes(),
            &ring(),
            &policy(Some("2026.11.01-2"), None)
        ),
        Err(BundleRejection::Rollback {
            offered: "2026.10.30-1".into(),
            newest: "2026.11.01-2".into()
        })
    );
    // The same version (re-verifying the cached bundle) and newer ones pass.
    assert!(accept_bundle(
        read(VALID).as_bytes(),
        &ring(),
        &policy(Some("2026.11.01-2"), None)
    )
    .is_ok());
    assert!(accept_bundle(
        read(VALID).as_bytes(),
        &ring(),
        &policy(Some("2026.10.30-1"), None)
    )
    .is_ok());
    // A garbage stored anchor anchors nothing.
    assert!(accept_bundle(older.as_bytes(), &ring(), &policy(Some("garbage"), None)).is_ok());
}

#[test]
fn max_age_expires_old_bundles_and_refuses_future_ones() {
    let day = 86_400;
    assert!(accept_bundle(
        read(VALID).as_bytes(),
        &ring(),
        &policy(None, Some(2 * day))
    )
    .is_ok());
    assert_eq!(
        accept_bundle(
            read(OLDER).as_bytes(),
            &ring(),
            &policy(None, Some(2 * day))
        ),
        Err(BundleRejection::Expired("2026-10-30T00:00:00.000Z".into()))
    );
    let early = BundlePolicy {
        now_unix_secs: NOW - 3 * day as i64,
        ..policy(None, Some(30 * day))
    };
    assert_eq!(
        accept_bundle(read(VALID).as_bytes(), &ring(), &early),
        Err(BundleRejection::SignedInFuture(
            "2026-11-01T00:00:00.000Z".into()
        ))
    );
}

#[test]
fn bundle_request_carries_bearer_and_etag_of_the_held_version() {
    let client = ClientInfo::ios("1.0.0", "3", "18.0");
    let r = build_config_bundle_request(" tok ", Some("2026.11.01-2"), &client).unwrap();
    assert_eq!(r.method, "GET");
    assert_eq!(r.path, CONFIG_BUNDLE_PATH);
    assert!(r.body.is_empty());
    let h = |n: &str| {
        r.headers
            .iter()
            .find(|(k, _)| k == n)
            .map(|(_, v)| v.as_str())
    };
    assert_eq!(h("Authorization"), Some("Bearer tok"));
    assert_eq!(h("If-None-Match"), Some("\"2026.11.01-2\""));
    assert_eq!(h("User-Agent"), Some("EyeGaze-iOS/1.0.0 (3; iOS 18.0)"));

    let none = build_config_bundle_request("tok", None, &client).unwrap();
    assert!(none.headers.iter().all(|(k, _)| k != "If-None-Match"));
    let junk = build_config_bundle_request("tok", Some("x\"y"), &client).unwrap();
    assert!(junk.headers.iter().all(|(k, _)| k != "If-None-Match"));
    assert_eq!(
        build_config_bundle_request("  ", None, &client),
        Err(SyncError::NotRegistered)
    );
}

#[test]
fn bundle_responses_classify() {
    let none: Vec<(String, String)> = Vec::new();
    assert_eq!(
        classify_config_bundle_response(200, &none),
        ConfigBundleOutcome::Fresh
    );
    assert_eq!(
        classify_config_bundle_response(304, &none),
        ConfigBundleOutcome::NotModified
    );
    assert_eq!(
        classify_config_bundle_response(404, &none),
        ConfigBundleOutcome::NoBundle
    );
    assert_eq!(
        classify_config_bundle_response(401, &none),
        ConfigBundleOutcome::Reauthenticate
    );
    let retry = vec![("retry-after".to_string(), "30".to_string())];
    assert_eq!(
        classify_config_bundle_response(503, &retry),
        ConfigBundleOutcome::RetryAfter(Some(30.0))
    );
    assert_eq!(
        classify_config_bundle_response(400, &none),
        ConfigBundleOutcome::Fatal { status: 400 }
    );
}

#[test]
fn version_order_is_day_then_counter() {
    assert_eq!(
        compare_bundle_versions("2026.11.01-2", "2026.10.30-1"),
        Some(Ordering::Greater)
    );
    assert_eq!(compare_bundle_versions("2026.11.01-2", "nope"), None);
}
