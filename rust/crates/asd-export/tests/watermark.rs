//! XC-6: the watermark every artifact carries and the mandatory ADR-0004 sentence.

use asd_export::claims;
use asd_export::policy::{Mode, Purpose};
use asd_export::watermark::{Watermark, MANDATORY_SENTENCE};

fn watermark() -> Watermark {
    Watermark {
        export_id: "0b9f6c1e-7a51-4c1e-9d43-2f1a7f0c9e11".into(),
        requested_by_display: "Dr. Sita Sharma".into(),
        requested_at_utc: "2026-10-31T09:15:00Z".into(),
        mode: Mode::Deidentified,
        purpose: Purpose::QualityImprovement,
        org_name: "Kathmandu Child Clinic".into(),
    }
}

#[test]
fn line_names_every_field() {
    let line = watermark().line();
    for part in [
        "0b9f6c1e-7a51-4c1e-9d43-2f1a7f0c9e11",
        "Dr. Sita Sharma",
        "2026-10-31T09:15:00Z",
        "de-identified",
        "quality improvement",
        "Kathmandu Child Clinic",
    ] {
        assert!(line.contains(part), "{part:?} missing from {line:?}");
    }
    assert_eq!(
        line,
        "Export 0b9f6c1e-7a51-4c1e-9d43-2f1a7f0c9e11 · Kathmandu Child Clinic · \
         de-identified · quality improvement · requested by Dr. Sita Sharma · \
         2026-10-31T09:15:00Z"
    );
}

#[test]
fn identified_mode_and_each_purpose_have_labels() {
    let mut w = watermark();
    w.mode = Mode::Identified;
    for (purpose, label) in [
        (Purpose::ClinicalCare, "clinical care"),
        (Purpose::Research, "research"),
        (Purpose::QualityImprovement, "quality improvement"),
        (Purpose::PatientRequest, "patient request"),
    ] {
        w.purpose = purpose;
        let line = w.line();
        assert!(line.contains(" · identified · "), "{line}");
        assert!(line.contains(label), "{line}");
    }
}

#[test]
fn lines_carry_the_mandatory_sentence() {
    assert_eq!(MANDATORY_SENTENCE, "This is not a diagnostic tool.");
    assert_eq!(MANDATORY_SENTENCE, claims::NOT_DIAGNOSTIC);
    let lines = watermark().lines();
    assert_eq!(lines[0], watermark().line());
    assert_eq!(lines[1], MANDATORY_SENTENCE);
}

#[test]
fn line_is_one_line_even_with_control_characters_in_the_input() {
    // A display name or org name with CR/LF/TAB must not break a CSV row, an HL7 NTE
    // segment or a PDF footer line.
    let mut w = watermark();
    w.requested_by_display = "Dr. A\r\nMSH|^~\\&|evil".into();
    w.org_name = "Clinic\tOne\u{0}".into();
    let line = w.line();
    assert!(!line.chars().any(char::is_control), "{line:?}");
    assert!(line.contains("Dr. A MSH|^~\\&|evil"), "{line:?}");
    assert!(line.contains("Clinic One"), "{line:?}");
}

#[test]
fn line_is_deterministic_and_serialises_with_wire_names() {
    assert_eq!(watermark().line(), watermark().line());
    let json = serde_json::to_value(watermark()).unwrap();
    assert_eq!(json["mode"], "deidentified");
    assert_eq!(json["purpose"], "quality_improvement");
    assert_eq!(json["export_id"], "0b9f6c1e-7a51-4c1e-9d43-2f1a7f0c9e11");
    let back: Watermark = serde_json::from_value(json).unwrap();
    assert_eq!(back, watermark());
}
