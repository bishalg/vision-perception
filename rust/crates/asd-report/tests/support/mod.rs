//! Shared fixtures for the `asd-report` golden tests.
//!
//! Goldens are rewritten with `ASD_WRITE_GOLDEN=1 cargo test -p asd-report`; review the
//! diff before committing.
#![allow(dead_code)] // each test binary uses a different subset

use std::collections::BTreeMap;
use std::path::PathBuf;

use asd_contract::session_v1_6::GazeSessionRecord;
use asd_report::mchat::{MchatResult, MchatRiskLevel};
use serde_json::{json, Value};

pub fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

pub fn golden_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/golden")
}

pub fn write_golden() -> bool {
    std::env::var("ASD_WRITE_GOLDEN").is_ok_and(|v| v == "1")
}

/// Compares `actual` with the golden file, or rewrites it under `ASD_WRITE_GOLDEN=1`.
pub fn assert_golden_bytes(name: &str, actual: &[u8]) {
    let path = golden_dir().join(name);
    if write_golden() {
        std::fs::write(&path, actual).expect("write golden");
        return;
    }
    let expected = std::fs::read(&path).unwrap_or_else(|_| {
        panic!("missing golden {name}; run with ASD_WRITE_GOLDEN=1 and review it")
    });
    if expected != actual {
        let out = std::env::temp_dir().join(format!("actual-{name}"));
        std::fs::write(&out, actual).ok();
        panic!(
            "{name} differs from the golden; actual written to {}",
            out.display()
        );
    }
}

/// The record behind `apps/ios_asd_gaze_app/Tests/Fixtures/expected-bundle.json`
/// (`FHIRBundleBuilderTests.goldenRecord`). Mouth dwell is absent and first-look latency is
/// present: neither may appear in the bundle (PLC-3, FHIR-3).
pub fn ios_fhir_golden_record() -> GazeSessionRecord {
    serde_json::from_value(json!({
        "id": "REC-GOLDEN-01",
        "sessionId": "GOLDEN01-SESSION",
        "participantId": "child-GOLDEN01",
        "captureSource": "truedepth",
        "startedAt": "2026-10-01T10:00:00.000Z",
        "endedAt": "2026-10-01T10:00:30.000Z",
        "durationSec": 30,
        "socialPreferenceScore": 0.42,
        "validFrameRatio": 0.9,
        "eyeDwellRatio": 0.55,
        "firstEyeLatencyMs": 310,
        "calibratedDistanceCm": 35,
        "qualityVerdict": "optimal",
        "socialSmileIntensity": 0.3,
        "smileReactivityIndex": 0.25,
        "flatAffectScore": 0.2,
        "surpriseScoreMean": 0.1,
        "schemaVersion": "v1.6",
        "zeroRawVideoEnforced": true,
        "perVideoComposite": 0.6,
        "perVideo": [
            {
                "stimulusId": "clip-a", "title": "Bubbles", "category": "social", "rows": [],
                "subscore": 0.7, "validSampleRatio": 0.9, "lowConfidence": false,
                "hasBaseline": true, "weight": 1
            },
            {
                "stimulusId": "clip-b", "title": "Untagged", "category": "geometric", "rows": [],
                "subscore": 0, "validSampleRatio": 0.9, "lowConfidence": false,
                "hasBaseline": false, "weight": 1
            }
        ]
    }))
    .expect("golden record decodes")
}

/// `FHIRBundleBuilderTests.goldenMCHAT`: 2 failed items (#2, #5), low, completed 09:00Z.
/// Answers as the fixture records them: every item `true` except #12.
pub fn ios_golden_mchat() -> (MchatResult, BTreeMap<u8, bool>) {
    let result = MchatResult {
        total_failed: 2,
        risk_level: MchatRiskLevel::Low,
        failed_question_ids: vec![2, 5],
        completion_date: "2026-10-01T09:00:00Z".into(),
    };
    let answers = (1..=20u8).map(|q| (q, q != 12)).collect();
    (result, answers)
}

/// `rawPayload` of `apps/ios_asd_gaze_app/Tests/Fixtures/expected-session-v1.6.json`
/// (the ingest oracle): a stored session as the backend keeps it in `sessions.summary`.
pub fn ios_session_record_value() -> Value {
    let path = repo_root().join("apps/ios_asd_gaze_app/Tests/Fixtures/expected-session-v1.6.json");
    let v: Value =
        serde_json::from_str(&std::fs::read_to_string(path).expect("iOS fixture")).expect("json");
    v["rawPayload"].clone()
}

pub fn ios_session_record() -> GazeSessionRecord {
    serde_json::from_value(ios_session_record_value()).expect("rawPayload decodes")
}

/// Record with selected fields replaced (JSON-level, so the contract type stays the judge).
pub fn record_with(base: &GazeSessionRecord, patch: Value) -> GazeSessionRecord {
    let mut v = serde_json::to_value(base).expect("encode");
    for (k, val) in patch.as_object().expect("object patch") {
        if val.is_null() {
            v.as_object_mut().unwrap().remove(k);
        } else {
            v[k] = val.clone();
        }
    }
    serde_json::from_value(v).expect("patched record decodes")
}

/// JSON equality where numbers compare by value (`24` == `24.0`): Swift and serde_json
/// spell whole doubles differently (trap 2), the content is what must match.
pub fn json_eq(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => x.as_f64() == y.as_f64(),
        (Value::Array(x), Value::Array(y)) => {
            x.len() == y.len() && x.iter().zip(y).all(|(p, q)| json_eq(p, q))
        }
        (Value::Object(x), Value::Object(y)) => {
            x.len() == y.len()
                && x.iter()
                    .all(|(k, v)| y.get(k).is_some_and(|w| json_eq(v, w)))
        }
        _ => a == b,
    }
}

// ---- PDF inspection (uncompressed content streams, literal strings) -------------------

/// Every string shown with `Tj`, decoded from WinAnsi, in drawing order. Handles literal
/// `(...)` strings and the `<hex>` strings `pdf-writer` uses for non-ASCII bytes.
pub fn pdf_text_runs(pdf: &[u8]) -> Vec<String> {
    let followed_by_tj = |end: usize| {
        let rest = &pdf[end..];
        let trimmed = rest
            .iter()
            .position(|c| !c.is_ascii_whitespace())
            .map_or(rest, |p| &rest[p..]);
        trimmed.starts_with(b"Tj")
    };
    let mut out = Vec::new();
    let mut i = 0;
    while i < pdf.len() {
        let parsed = match pdf[i] {
            b'(' => Some(literal_string(pdf, i + 1)),
            b'<' if pdf.get(i + 1) != Some(&b'<') && (i == 0 || pdf[i - 1] != b'<') => {
                hex_string(pdf, i + 1)
            }
            _ => None,
        };
        match parsed {
            Some((s, end)) => {
                if followed_by_tj(end) {
                    out.push(s.iter().map(|&b| winansi_char(b)).collect());
                }
                i = end;
            }
            None => i += 1,
        }
    }
    out
}

/// Parses a hex string starting after `<`; `None` when it is not one.
fn hex_string(pdf: &[u8], start: usize) -> Option<(Vec<u8>, usize)> {
    let len = pdf[start..].iter().position(|&c| c == b'>')?;
    let digits: Vec<u8> = pdf[start..start + len]
        .iter()
        .copied()
        .filter(|c| !c.is_ascii_whitespace())
        .collect();
    if !digits.iter().all(u8::is_ascii_hexdigit) || !digits.len().is_multiple_of(2) {
        return None;
    }
    let bytes = digits
        .chunks(2)
        .map(|p| u8::from_str_radix(std::str::from_utf8(p).unwrap(), 16).unwrap())
        .collect();
    Some((bytes, start + len + 1))
}

/// Parses a PDF literal string starting after `(`; returns the bytes and the index after `)`.
fn literal_string(pdf: &[u8], mut i: usize) -> (Vec<u8>, usize) {
    let mut out = Vec::new();
    let mut depth = 1;
    while i < pdf.len() {
        let c = pdf[i];
        match c {
            b'\\' => {
                i += 1;
                let e = pdf[i];
                match e {
                    b'n' => out.push(b'\n'),
                    b'r' => out.push(b'\r'),
                    b't' => out.push(b'\t'),
                    b'b' => out.push(8),
                    b'f' => out.push(12),
                    b'0'..=b'7' => {
                        let mut v = 0u32;
                        let mut n = 0;
                        while n < 3 && i < pdf.len() && (b'0'..=b'7').contains(&pdf[i]) {
                            v = v * 8 + u32::from(pdf[i] - b'0');
                            i += 1;
                            n += 1;
                        }
                        out.push(u8::try_from(v & 0xff).unwrap());
                        continue;
                    }
                    other => out.push(other),
                }
            }
            b'(' => {
                depth += 1;
                out.push(c);
            }
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return (out, i + 1);
                }
                out.push(c);
            }
            _ => out.push(c),
        }
        i += 1;
    }
    (out, i)
}

fn winansi_char(b: u8) -> char {
    match b {
        0x80 => '€',
        0x85 => '…',
        0x91 => '\u{2018}',
        0x92 => '\u{2019}',
        0x93 => '\u{201C}',
        0x94 => '\u{201D}',
        0x95 => '•',
        0x96 => '–',
        0x97 => '—',
        _ => char::from(b),
    }
}

/// Number of `/Type /Page` objects (not `/Pages`).
pub fn pdf_page_count(pdf: &[u8]) -> usize {
    let hay = String::from_utf8_lossy(pdf);
    hay.matches("/Type /Page").count() - hay.matches("/Type /Pages").count()
}
