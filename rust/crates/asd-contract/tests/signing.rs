//! owner: WP-R.3
//!
//! CON-6: Ed25519 over the JCS (RFC 8785) form of the bundle without `signature`, encoded
//! `ed25519:` + base64url without padding; tamper detection; key rotation by `key_id`; and
//! the JCS and Ed25519 reference vectors from RFC 8785 and RFC 8032.

use std::path::PathBuf;

use asd_contract::config::{ConfigBundle, ScoringProfile};
use asd_contract::signing::{self, SigningError, SIGNATURE_PREFIX};
use serde_json::{json, Value};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

fn unsigned_bundle(key_id: &str) -> ConfigBundle {
    let res = repo_root().join("domains/asd_screening/ios/Sources/CurvAsdGazeKit/Resources");
    ConfigBundle {
        bundle_version: "2026.10.30-1".into(),
        key_id: key_id.into(),
        catalog: serde_json::from_str(
            &std::fs::read_to_string(res.join("stimulus_catalog.json")).unwrap(),
        )
        .unwrap(),
        baselines: serde_json::from_str(
            &std::fs::read_to_string(res.join("normative_baselines.json")).unwrap(),
        )
        .unwrap(),
        scoring_profile: ScoringProfile::default(),
        feature_flags: Default::default(),
        signed_at: "2026-10-30T12:00:00.000Z".into(),
        signature: String::new(),
    }
}

fn signed_json(seed: u8, key_id: &str) -> (String, signing::VerifyingKey) {
    let key = signing::signing_key_from_seed([seed; 32]);
    let mut bundle = unsigned_bundle(key_id);
    signing::sign_bundle(&mut bundle, &key).unwrap();
    (
        serde_json::to_string_pretty(&bundle).unwrap(),
        key.verifying_key(),
    )
}

#[test]
fn sign_then_verify_round_trips() {
    let (json, vk) = signed_json(7, "k1");
    let bundle = signing::verify_bundle(&json, &vk).expect("valid signature");
    assert_eq!(bundle.bundle_version, "2026.10.30-1");
    assert_eq!(bundle.scoring_profile, ScoringProfile::default());
}

#[test]
fn signature_is_prefixed_base64url_without_padding() {
    let (json, _) = signed_json(7, "k1");
    let v: Value = serde_json::from_str(&json).unwrap();
    let sig = v["signature"].as_str().unwrap();
    let body = sig.strip_prefix(SIGNATURE_PREFIX).expect("ed25519: prefix");
    assert_eq!(SIGNATURE_PREFIX, "ed25519:");
    assert_eq!(body.len(), 86, "64 bytes, base64url, no padding");
    assert!(body
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'));
}

#[test]
fn signature_covers_the_jcs_form_without_the_signature_member() {
    let key = signing::signing_key_from_seed([9; 32]);
    let mut bundle = unsigned_bundle("k1");
    signing::sign_bundle(&mut bundle, &key).unwrap();
    let mut v = serde_json::to_value(&bundle).unwrap();
    v.as_object_mut().unwrap().remove("signature");
    let payload = signing::canonical_bytes(&v).unwrap();
    assert_eq!(payload, signing::signing_payload(&v).unwrap());

    use ed25519_dalek::Signer;
    let expected = key.sign(&payload);
    let encoded = signing::encode_signature(&expected);
    assert_eq!(bundle.signature, encoded);
}

#[test]
fn hard_to_parse_doubles_survive_sign_serialise_parse_verify() {
    // The verifier re-parses the signed text. serde_json's default float parser can land
    // one ULP off for 17-digit values, which changes the canonical form and fails a valid
    // signature; asd-contract enables `float_roundtrip` to prevent it.
    let key = signing::signing_key_from_seed([4; 32]);
    let tricky = [
        f64::from_bits(0x41b3de4355555555), // 333333333.3333333
        f64::from_bits(0x41b3de4355555557), // 333333333.33333343
        f64::from_bits(0x44b52d02c7e14af7), // 1.0000000000000001e+23
        0.1 + 0.2,
        2.0 / 3.0,
        5e-324,
        f64::MAX,
    ];
    for x in tricky {
        let mut bundle = unsigned_bundle("k1");
        bundle.scoring_profile.flag.green_floor = x;
        bundle.scoring_profile.low_confidence_valid_ratio = x / 7.0;
        signing::sign_bundle(&mut bundle, &key).unwrap();
        let text = serde_json::to_string(&bundle).unwrap();
        let back = signing::verify_bundle(&text, &key.verifying_key())
            .unwrap_or_else(|e| panic!("{x:e}: {e}"));
        assert_eq!(back.scoring_profile.flag.green_floor.to_bits(), x.to_bits());
    }
}

#[test]
fn whitespace_and_key_order_do_not_matter() {
    let (json, vk) = signed_json(7, "k1");
    let v: Value = serde_json::from_str(&json).unwrap();
    let compact = serde_json::to_string(&v).unwrap();
    signing::verify_bundle(&compact, &vk).expect("compact form verifies");
}

#[test]
fn any_tampering_is_rejected() {
    let (json, vk) = signed_json(7, "k1");
    let original: Value = serde_json::from_str(&json).unwrap();
    type Edit = Box<dyn Fn(&mut Value)>;
    let edits: Vec<Edit> = vec![
        Box::new(|v| v["scoring_profile"]["flag"]["greenFloor"] = json!(0.66)),
        Box::new(|v| v["bundle_version"] = json!("2026.10.30-2")),
        Box::new(|v| v["key_id"] = json!("k2")),
        Box::new(|v| v["feature_flags"] = json!({"norm_collection": true})),
        Box::new(|v| v["catalog"]["clips"][0]["durationSec"] = json!(9.5)),
        Box::new(|v| v["extra_member"] = json!(1)),
    ];
    for edit in edits {
        let mut v = original.clone();
        edit(&mut v);
        let err = signing::verify_bundle(&v.to_string(), &vk).unwrap_err();
        assert!(matches!(err, SigningError::BadSignature), "{err:?}");
    }
}

#[test]
fn flipping_one_byte_of_the_signature_is_rejected() {
    let (json, vk) = signed_json(7, "k1");
    let mut v: Value = serde_json::from_str(&json).unwrap();
    let sig = v["signature"].as_str().unwrap().to_string();
    let mut chars: Vec<char> = sig.chars().collect();
    let i = SIGNATURE_PREFIX.len() + 10;
    chars[i] = if chars[i] == 'A' { 'B' } else { 'A' };
    v["signature"] = json!(chars.into_iter().collect::<String>());
    assert!(signing::verify_bundle(&v.to_string(), &vk).is_err());
}

#[test]
fn malformed_signatures_and_documents_are_errors_not_panics() {
    let (json, vk) = signed_json(7, "k1");
    let mut v: Value = serde_json::from_str(&json).unwrap();
    for bad in [
        json!("rsa:abc"),
        json!("ed25519:!!!"),
        json!("ed25519:"),
        json!(5),
    ] {
        v["signature"] = bad;
        assert!(signing::verify_bundle(&v.to_string(), &vk).is_err());
    }
    v.as_object_mut().unwrap().remove("signature");
    assert!(matches!(
        signing::verify_bundle(&v.to_string(), &vk),
        Err(SigningError::MissingSignature)
    ));
    assert!(signing::verify_bundle("not json", &vk).is_err());
    assert!(signing::verify_bundle("[]", &vk).is_err());
}

#[test]
fn a_different_key_is_rejected() {
    let (json, _) = signed_json(7, "k1");
    let other = signing::signing_key_from_seed([8; 32]).verifying_key();
    assert!(matches!(
        signing::verify_bundle(&json, &other),
        Err(SigningError::BadSignature)
    ));
}

#[test]
fn key_rotation_selects_the_key_by_key_id() {
    let old = signing::signing_key_from_seed([1; 32]).verifying_key();
    let new = signing::signing_key_from_seed([2; 32]).verifying_key();
    let keyring = [("k-old", old), ("k-new", new)];

    let (by_old, _) = signed_json(1, "k-old");
    let (by_new, _) = signed_json(2, "k-new");
    signing::verify_bundle_with_keys(&by_old, &keyring).expect("old key still trusted");
    signing::verify_bundle_with_keys(&by_new, &keyring).expect("new key trusted");

    let (unknown, _) = signed_json(3, "k-retired");
    assert!(matches!(
        signing::verify_bundle_with_keys(&unknown, &keyring),
        Err(SigningError::UnknownKeyId(id)) if id == "k-retired"
    ));

    // A bundle claiming the new key id but signed by the old key fails.
    let (lying, _) = signed_json(1, "k-new");
    assert!(matches!(
        signing::verify_bundle_with_keys(&lying, &keyring),
        Err(SigningError::BadSignature)
    ));
}

#[test]
fn keys_encode_and_parse() {
    let key = signing::signing_key_from_seed([5; 32]);
    let vk = key.verifying_key();
    let b64url = signing::encode_public_key(&vk);
    assert_eq!(b64url.len(), 43);
    assert_eq!(signing::parse_public_key(&b64url).unwrap(), vk);
    // Standard alphabet with padding is accepted too (keys pasted from other tools).
    use base64::Engine;
    let std_b64 = base64::engine::general_purpose::STANDARD.encode(vk.as_bytes());
    assert_eq!(signing::parse_public_key(&std_b64).unwrap(), vk);
    assert!(signing::parse_public_key("AAAA").is_err(), "wrong length");
    assert!(signing::parse_public_key("***").is_err());

    let pem = signing::signing_key_to_pkcs8_pem(&key).unwrap();
    assert!(pem.starts_with("-----BEGIN PRIVATE KEY-----"));
    let parsed = signing::parse_signing_key(&pem).unwrap();
    assert_eq!(parsed.to_bytes(), key.to_bytes());
    let seed_b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(key.to_bytes());
    assert_eq!(
        signing::parse_signing_key(&seed_b64).unwrap().to_bytes(),
        key.to_bytes()
    );
    assert!(signing::parse_signing_key("garbage").is_err());

    let id = signing::key_id(&vk);
    assert!(
        id.starts_with("ed25519-") && id.len() == "ed25519-".len() + 16,
        "{id}"
    );
    assert_eq!(id, signing::key_id(&vk), "stable");
}

// ---- RFC 8032 §7.1 test 1 --------------------------------------------------------------

fn hex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

#[test]
fn rfc8032_test_vector_1() {
    let seed: [u8; 32] = hex("9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60")
        .try_into()
        .unwrap();
    let key = signing::signing_key_from_seed(seed);
    assert_eq!(
        key.verifying_key().as_bytes().to_vec(),
        hex("d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a")
    );
    use ed25519_dalek::Signer;
    assert_eq!(
        key.sign(b"").to_bytes().to_vec(),
        hex("e5564300c360ac729086e2cc806e828a84877f1eb8e5d974d873e065224901555fb8821590a33bacc61e39701cf9b46bd25bf5f0595bbe24655141438e7a100b")
    );
}

// ---- RFC 8785 vectors ------------------------------------------------------------------

#[test]
fn rfc8785_section_3_2_2_example() {
    // The RFC writes the string member with `\u` escapes for the euro sign, `B`, `"` and
    // `\`; the same characters appear here literally or with short escapes. Parsing escapes
    // is serde_json's job; what is tested is the canonical output.
    let input = r#"{
      "numbers": [333333333.33333329, 1E30, 4.50, 2e-3, 0.000000000000000000000000001],
      "string": "€$\u000F\u000aA'B\"\\\\\"\/",
      "literals": [null, true, false]
    }"#;
    let v: Value = serde_json::from_str(input).unwrap();
    let out = String::from_utf8(signing::canonical_bytes(&v).unwrap()).unwrap();
    assert_eq!(
        out,
        r#"{"literals":[null,true,false],"numbers":[333333333.3333333,1e+30,4.5,0.002,1e-27],"string":"€$\u000f\nA'B\"\\\\\"/"}"#
    );
}

#[test]
fn rfc8785_section_3_2_3_sorts_by_utf16_code_units() {
    let input = r#"{
      "€": "Euro Sign",
      "\r": "Carriage Return",
      "דּ": "Hebrew Letter Dalet With Dagesh",
      "1": "One",
      "😀": "Emoji: Grinning Face",
      "\u0080": "Control",
      "ö": "Latin Small Letter O With Diaeresis"
    }"#;
    let v: Value = serde_json::from_str(input).unwrap();
    let out: Value =
        serde_json::from_slice(&signing::canonical_bytes(&v).unwrap()).unwrap_or(Value::Null);
    let text = String::from_utf8(signing::canonical_bytes(&v).unwrap()).unwrap();
    let order: Vec<&str> = [
        "Carriage Return",
        "One",
        "Control",
        "Latin Small Letter O With Diaeresis",
        "Euro Sign",
        "Emoji: Grinning Face",
        "Hebrew Letter Dalet With Dagesh",
    ]
    .to_vec();
    let positions: Vec<usize> = order.iter().map(|s| text.find(s).unwrap()).collect();
    assert!(
        positions.windows(2).all(|w| w[0] < w[1]),
        "UTF-16 order violated: {text}"
    );
    assert!(out.is_object());
}

#[test]
fn rfc8785_appendix_b_numbers() {
    let cases: [(u64, &str); 23] = [
        (0x0000000000000000, "0"),
        (0x8000000000000000, "0"),
        (0x0000000000000001, "5e-324"),
        (0x8000000000000001, "-5e-324"),
        (0x7fefffffffffffff, "1.7976931348623157e+308"),
        (0x4340000000000000, "9007199254740992"),
        (0xc340000000000000, "-9007199254740992"),
        (0x4430000000000000, "295147905179352830000"),
        (0x44b52d02c7e14af5, "9.999999999999997e+22"),
        (0x44b52d02c7e14af6, "1e+23"),
        (0x44b52d02c7e14af7, "1.0000000000000001e+23"),
        (0x444b1ae4d6e2ef4e, "999999999999999700000"),
        (0x444b1ae4d6e2ef4f, "999999999999999900000"),
        (0x444b1ae4d6e2ef50, "1e+21"),
        (0x3eb0c6f7a0b5ed8c, "9.999999999999997e-7"),
        (0x3eb0c6f7a0b5ed8d, "0.000001"),
        (0x41b3de4355555553, "333333333.3333332"),
        (0x41b3de4355555554, "333333333.33333325"),
        (0x41b3de4355555555, "333333333.3333333"),
        (0x41b3de4355555556, "333333333.3333334"),
        (0x41b3de4355555557, "333333333.33333343"),
        (0xbecbf647612f3696, "-0.0000033333333333333333"),
        (0x43143ff3c1cb0959, "1424953923781206.2"),
    ];
    for (bits, expected) in cases {
        let out = signing::canonical_bytes(&f64::from_bits(bits)).unwrap();
        assert_eq!(
            String::from_utf8(out).unwrap(),
            expected,
            "bits {bits:#018x}"
        );
    }
}
