//! XC-7: every external code lives in one table with a `verified` flag and a source note;
//! encoders cannot emit an unverified code without the profile's opt-in.

use std::collections::BTreeSet;
use std::path::PathBuf;

use asd_export::codes::{self, Code, CodePolicy, UnverifiedCode};
use serde_json::Value;

#[test]
fn every_code_has_a_source_note_and_display() {
    assert!(!codes::ALL.is_empty());
    for code in codes::ALL {
        assert!(
            !code.source.trim().is_empty(),
            "{code:?} has no source note"
        );
        assert!(!code.display.trim().is_empty(), "{code:?} has no display");
        assert!(!code.code.trim().is_empty());
        assert!(
            code.system.starts_with("http://") || code.system.starts_with("https://"),
            "{code:?}: system is not an absolute URI"
        );
    }
}

#[test]
fn codes_are_unique_per_system() {
    let mut seen = BTreeSet::new();
    for code in codes::ALL {
        assert!(
            seen.insert((code.system, code.code)),
            "duplicate {}|{}",
            code.system,
            code.code
        );
    }
}

#[test]
fn loinc_gaze_panel_is_unverified() {
    // G8 / ADR-0008 Q6: nobody has checked 93044-6 against the LOINC release.
    let code = codes::lookup(codes::system::LOINC, "93044-6").expect("93044-6 in the table");
    assert!(!code.verified);
    assert_eq!(code, &codes::LOINC_GAZE_PANEL);
}

#[test]
fn ihe_xfdf_format_code_is_unverified() {
    // G7: `urn:ihe:iti:xfdf:2008` is XFDF, not PDF; only the parity bundle carries it.
    let code = codes::lookup(codes::system::IHE_FORMAT, "urn:ihe:iti:xfdf:2008").unwrap();
    assert!(!code.verified);
}

#[test]
fn require_refuses_an_unverified_code_without_the_opt_in() {
    let strict = CodePolicy::default();
    assert!(!strict.allow_unverified);
    let err = codes::LOINC_GAZE_PANEL.require(strict).unwrap_err();
    assert_eq!(
        err,
        UnverifiedCode {
            system: codes::system::LOINC,
            code: "93044-6"
        }
    );
    assert!(err.to_string().contains("93044-6"));

    let opt_in = CodePolicy {
        allow_unverified: true,
    };
    assert_eq!(
        codes::LOINC_GAZE_PANEL.require(opt_in).unwrap(),
        &codes::LOINC_GAZE_PANEL
    );
}

#[test]
fn require_passes_a_verified_code_without_the_opt_in() {
    let verified: Vec<&Code> = codes::ALL.iter().copied().filter(|c| c.verified).collect();
    assert!(!verified.is_empty(), "the table has verified codes");
    for code in verified {
        assert_eq!(code.require(CodePolicy::default()).unwrap(), code);
    }
}

/// Every external (non-`curv.org`) coding the session bundle emits is in the table, so the
/// table is the complete inventory XC-7 asks for. Uses the B.8 golden and the iOS fixture.
#[test]
fn every_external_coding_in_the_session_bundles_is_in_the_table() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let files = [
        root.join("tests/golden/server-bundle.json"),
        root.join("../../../apps/ios_asd_gaze_app/Tests/Fixtures/expected-bundle.json"),
    ];
    let mut found = BTreeSet::new();
    for file in files {
        let text = std::fs::read_to_string(&file)
            .unwrap_or_else(|e| panic!("read {}: {e}", file.display()));
        let bundle: Value = serde_json::from_str(&text).unwrap();
        collect_codings(&bundle, &mut found);
    }
    let external: Vec<_> = found
        .into_iter()
        .filter(|(system, _)| !system.starts_with(codes::system::LOCAL))
        .collect();
    assert!(external.len() >= 10, "expected the bundle's external codes");
    for (system, code) in external {
        assert!(
            codes::lookup(&system, &code).is_some(),
            "{system}|{code} is emitted but missing from codes.rs"
        );
    }
}

fn collect_codings(v: &Value, out: &mut BTreeSet<(String, String)>) {
    match v {
        Value::Object(map) => {
            if let (Some(Value::String(s)), Some(Value::String(c))) =
                (map.get("system"), map.get("code"))
            {
                out.insert((s.clone(), c.clone()));
            }
            map.values().for_each(|x| collect_codings(x, out));
        }
        Value::Array(items) => items.iter().for_each(|x| collect_codings(x, out)),
        _ => {}
    }
}
