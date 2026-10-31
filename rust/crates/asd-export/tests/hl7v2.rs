//! WP-I.4 HL7 v2.5.1 ORU^R01 encoder (HL7-1..7). Golden messages are CR-separated text in
//! `tests/golden/hl7v2/` (kept byte-exact by `.gitattributes`). Regenerate with
//! `UPDATE_GOLDEN=1 cargo test -p asd-export --test hl7v2` and review the diff.

mod export_support;

use std::collections::BTreeMap;
use std::path::PathBuf;

use asd_export::codes::CodePolicy;
use asd_export::hl7v2::escape::{escape, unescape};
use asd_export::hl7v2::{
    encode_oru_r01, file_name, status_letter, Hl7Error, Hl7Options, Hl7Profile, MappedCode,
    Pid5Mode,
};
use asd_export::model::{
    ClinicalReviewSummary, Disposition, ExportRecord, Mrn, ReportStatus, ReviewState, Signature,
    SignatureRole,
};
use asd_export::policy::{Mode, Purpose};
use asd_export::watermark::{Watermark, MANDATORY_SENTENCE};
use asd_export::ExportError;
use export_support::golden_record;

const GENERATED_AT: &str = "2026-10-05T04:15:00Z";

fn profile() -> Hl7Profile {
    Hl7Profile {
        sending_app: "ASD".into(),
        sending_facility: "KCC".into(),
        receiving_app: "HIS".into(),
        receiving_facility: "TUTH".into(),
        ..Hl7Profile::default()
    }
}

fn watermark(mode: Mode) -> Watermark {
    Watermark {
        export_id: "exp-0001".into(),
        requested_by_display: "Dr. A. Sharma".into(),
        requested_at_utc: "2026-10-05T04:10:00Z".into(),
        mode,
        purpose: Purpose::ClinicalCare,
        org_name: "Kathmandu Child Clinic".into(),
    }
}

fn identified() -> ExportRecord {
    let mut r = golden_record();
    r.patient.mrn = Some(Mrn {
        value: "MRN-12345".into(),
        assigning_authority: "TUTH".into(),
    });
    r
}

fn review(state: ReviewState, amended: bool) -> ClinicalReviewSummary {
    ClinicalReviewSummary {
        state,
        version_no: if amended { 2 } else { 1 },
        amended,
        disposition: Some(Disposition::Monitor),
        signed_at: Some("2026-10-02T08:30:00Z".into()),
        content_hash: None,
        signatures: vec![Signature {
            role: SignatureRole::Sign,
            user_id: Some("u-77".into()),
            display_name: Some("Dr. Sita Rai".into()),
            professional_registration: Some("NMC 12345".into()),
            signed_at: "2026-10-02T08:30:00Z".into(),
        }],
        impression: Some(
            "Attention to faces was lower than the age reference.\n\nRe-check in 3 months.".into(),
        ),
        recommendations: Some("Repeat observation.".into()),
    }
}

fn encode(record: &ExportRecord, profile: &Hl7Profile) -> Result<String, ExportError> {
    encode_with(record, profile, None)
}

fn encode_with(
    record: &ExportRecord,
    profile: &Hl7Profile,
    pdf: Option<&[u8]>,
) -> Result<String, ExportError> {
    let wm = watermark(Mode::Identified);
    let bytes = encode_oru_r01(
        record,
        &Hl7Options {
            profile,
            delivery_id: "dlv-0001",
            generated_at: GENERATED_AT,
            watermark: &wm,
            pdf,
        },
    )?;
    Ok(String::from_utf8(bytes).expect("UTF-8"))
}

fn segments(msg: &str) -> Vec<&str> {
    assert!(msg.ends_with('\r'), "message ends with CR");
    assert!(!msg.contains('\n'), "no LF anywhere");
    msg.split_terminator('\r').collect()
}

fn find<'a>(msg: &'a str, name: &str) -> Vec<&'a str> {
    segments(msg)
        .into_iter()
        .filter(|s| s.split('|').next() == Some(name))
        .collect()
}

fn golden(name: &str, actual: &str) {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden/hl7v2")
        .join(name);
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, actual).unwrap();
    }
    let expected = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{name}: {e}"));
    assert_eq!(
        actual, expected,
        "golden {name} (UPDATE_GOLDEN=1 to refresh)"
    );
}

#[test]
fn golden_unsigned() {
    golden("unsigned.hl7", &encode(&identified(), &profile()).unwrap());
}

#[test]
fn golden_final() {
    let mut r = identified();
    r.review = Some(review(ReviewState::Final, false));
    let mut p = profile();
    p.send_pv1 = true;
    golden("final.hl7", &encode(&r, &p).unwrap());
}

#[test]
fn golden_amended() {
    let mut r = identified();
    r.review = Some(review(ReviewState::Final, true));
    golden("amended.hl7", &encode(&r, &profile()).unwrap());
}

#[test]
fn deidentified_is_refused() {
    let wm = watermark(Mode::Deidentified);
    let p = profile();
    let err = encode_oru_r01(
        &identified(),
        &Hl7Options {
            profile: &p,
            delivery_id: "d",
            generated_at: GENERATED_AT,
            watermark: &wm,
            pdf: None,
        },
    )
    .unwrap_err();
    assert!(matches!(
        err,
        ExportError::Hl7(Hl7Error::DeidentifiedRefused)
    ));
    // A record the de-identifier already processed is refused even with a wrong watermark.
    let mut r = identified();
    r.days_since_first_session = Some(0);
    assert!(matches!(
        encode(&r, &p).unwrap_err(),
        ExportError::Hl7(Hl7Error::DeidentifiedRefused)
    ));
}

#[test]
fn refuses_without_mrn() {
    let err = encode(&golden_record(), &profile()).unwrap_err();
    assert!(
        matches!(err, ExportError::Hl7(Hl7Error::MissingMrn)),
        "{err}"
    );
}

#[test]
fn msh_has_the_standard_shape() {
    let msg = encode(&identified(), &profile()).unwrap();
    let msh = find(&msg, "MSH")[0];
    assert!(msh.starts_with("MSH|^~\\&|ASD|KCC|HIS|TUTH|"), "{msh}");
    let f: Vec<&str> = msh.split('|').collect();
    // f[0] is "MSH"; MSH-n is f[n-1] because MSH-1 is the separator itself.
    assert_eq!(f[1], "^~\\&");
    assert_eq!(f[8], "ORU^R01^ORU_R01");
    assert_eq!(f[9], "dlv-0001");
    assert_eq!(f[10], "P");
    assert_eq!(f[11], "2.5.1");
    assert_eq!(f[17], "UNICODE UTF-8");
    assert_eq!(f.len(), 18, "no fields past MSH-18");
    // 2026-10-05T04:15:00Z in Asia/Kathmandu.
    assert_eq!(f[6], "20261005100000+0545");
}

#[test]
fn test_destination_sets_msh_11_t() {
    let mut p = profile();
    p.test_destination = true;
    let msg = encode(&identified(), &p).unwrap();
    assert_eq!(find(&msg, "MSH")[0].split('|').nth(10), Some("T"));
}

#[test]
fn ts_uses_the_org_zone_with_offset() {
    for (tz, expect) in [
        ("Asia/Kathmandu", "+0545"),
        ("Asia/Kolkata", "+0530"),
        ("UTC", "+0000"),
    ] {
        let mut r = identified();
        r.org.timezone = tz.into();
        let msg = encode(&r, &profile()).unwrap();
        let msh = find(&msg, "MSH")[0];
        assert!(
            msh.split('|').nth(6).unwrap().ends_with(expect),
            "{tz}: {msh}"
        );
        let obr = find(&msg, "OBR")[0];
        let start = obr.split('|').nth(7).unwrap();
        assert_eq!(start.len(), 19, "{start}");
        assert!(start.ends_with(expect), "{tz}: {start}");
    }
}

#[test]
fn unknown_zone_is_an_error_not_utc() {
    let mut r = identified();
    r.org.timezone = "Mars/Olympus".into();
    assert!(matches!(
        encode(&r, &profile()).unwrap_err(),
        ExportError::InvalidTimezone(_)
    ));
}

#[test]
fn pid_carries_mrn_authority_and_policy() {
    let msg = encode(&identified(), &profile()).unwrap();
    let pid = find(&msg, "PID")[0];
    let f: Vec<&str> = pid.split('|').collect();
    assert_eq!(f[3], "MRN-12345^^^TUTH^MR");
    assert!(f.get(5).is_none_or(|v| v.is_empty()), "PID-5 empty: {pid}");

    let mut p = profile();
    p.pid5_mode = Pid5Mode::Token;
    let r = identified();
    let msg = encode(&r, &p).unwrap();
    let f5 = find(&msg, "PID")[0].split('|').nth(5).unwrap().to_owned();
    assert_eq!(f5, r.patient.participant_token);
}

#[test]
fn pv1_only_when_the_profile_asks() {
    let none = encode(&identified(), &profile()).unwrap();
    assert!(find(&none, "PV1").is_empty());
    let mut p = profile();
    p.send_pv1 = true;
    let some = encode(&identified(), &p).unwrap();
    assert_eq!(find(&some, "PV1"), vec!["PV1|1|O"]);
}

#[test]
fn status_letters_follow_the_review() {
    assert_eq!(status_letter(ReportStatus::Preliminary), 'P');
    assert_eq!(status_letter(ReportStatus::Final), 'F');
    assert_eq!(status_letter(ReportStatus::Amended), 'C');
    for (review_state, expect) in [
        (None, 'P'),
        (Some(review(ReviewState::InProgress, false)), 'P'),
        (Some(review(ReviewState::AwaitingCosign, false)), 'P'),
        (Some(review(ReviewState::Final, false)), 'F'),
        (Some(review(ReviewState::Final, true)), 'C'),
    ] {
        let mut r = identified();
        r.review = review_state;
        let msg = encode(&r, &profile()).unwrap();
        let obr: Vec<&str> = find(&msg, "OBR")[0].split('|').collect();
        assert_eq!(obr[25], expect.to_string());
        for obx in find(&msg, "OBX") {
            assert_eq!(
                obx.split('|').nth(11),
                Some(expect.to_string().as_str()),
                "{obx}"
            );
        }
    }
}

#[test]
fn obr_names_the_service_signer_and_times() {
    let mut r = identified();
    r.review = Some(review(ReviewState::Final, false));
    let msg = encode(&r, &profile()).unwrap();
    let obr: Vec<&str> = find(&msg, "OBR")[0].split('|').collect();
    assert_eq!(obr[4], "ASD-GAZE^Gaze engagement observation review^L");
    assert_eq!(obr[22], "20261002141500+0545");
    assert_eq!(obr[32], "u-77&Dr. Sita Rai");
}

#[test]
fn loinc_alternate_only_when_verified_or_opted_in() {
    let default = encode(&identified(), &profile()).unwrap();
    assert!(
        !default.contains("93044-6"),
        "G8: unverified LOINC is not sent"
    );
    let mut p = profile();
    p.code_policy = CodePolicy {
        allow_unverified: true,
    };
    let opted = encode(&identified(), &p).unwrap();
    let obr = find(&opted, "OBR")[0];
    assert!(
        obr.contains("^L^93044-6^Gaze tracking assessment panel^LN"),
        "{obr}"
    );
}

#[test]
fn absent_metrics_produce_no_obx() {
    let mut r = identified();
    r.session.eye_dwell = None;
    r.session.mouth_dwell = None;
    r.session.social_preference = None;
    r.session.valid_frame_ratio = None;
    r.session.summary.composite_score = None;
    r.clips.clear();
    let msg = encode(&r, &profile()).unwrap();
    assert!(find(&msg, "OBX").is_empty(), "{msg}");

    let full = encode(&identified(), &profile()).unwrap();
    let codes: Vec<&str> = find(&full, "OBX")
        .iter()
        .map(|o| o.split('|').nth(3).unwrap().split('^').next().unwrap())
        .collect();
    assert_eq!(
        &codes[..3],
        [
            "social-preference-score",
            "eye-region-dwell-ratio",
            "valid-frame-ratio"
        ]
    );
    // The golden session has no mouth dwell: that metric is absent, not zero.
    assert!(!codes.contains(&"mouth-region-dwell-ratio"));
    assert!(!codes.contains(&"first-eye-latency"), "PLC-1");
}

#[test]
fn numeric_obx_has_nm_type_and_ucum_units() {
    let msg = encode(&identified(), &profile()).unwrap();
    let obx: Vec<&str> = find(&msg, "OBX")[0].split('|').collect();
    assert_eq!(obx[1], "1");
    assert_eq!(obx[2], "NM");
    assert_eq!(
        obx[3],
        "social-preference-score^Social visual preference score^L"
    );
    assert_eq!(obx[6], "1^score^UCUM");
    assert!(obx[5].parse::<f64>().is_ok());
}

#[test]
fn code_map_overrides_obx_3() {
    let mut p = profile();
    p.code_map = BTreeMap::from([(
        "eye-region-dwell-ratio".to_owned(),
        MappedCode {
            code: "H-EYE".into(),
            text: "Eye dwell".into(),
            system: "99TUTH".into(),
        },
    )]);
    let msg = encode(&identified(), &p).unwrap();
    let obxs = find(&msg, "OBX");
    assert!(obxs.iter().any(|o| o.contains("|H-EYE^Eye dwell^99TUTH|")));
    assert!(!obxs.iter().any(|o| o.contains("|eye-region-dwell-ratio^")));
    assert!(obxs.iter().any(|o| o.contains("|valid-frame-ratio^")));
}

#[test]
fn disposition_and_impression_paragraphs() {
    let mut r = identified();
    r.review = Some(review(ReviewState::Final, false));
    let msg = encode(&r, &profile()).unwrap();
    let obxs = find(&msg, "OBX");
    let disp = obxs.iter().find(|o| o.contains("|CWE|")).unwrap();
    assert!(disp.contains("|disposition^Clinician disposition^L|"));
    assert!(disp.contains("|monitor^Monitor^L|"));
    let tx: Vec<&&str> = obxs.iter().filter(|o| o.contains("|TX|")).collect();
    assert_eq!(tx.len(), 2, "one TX per paragraph");
    assert!(tx[1].contains("Re-check in 3 months."));

    // Free text the backend left out stays out.
    r.review.as_mut().unwrap().impression = None;
    let msg = encode(&r, &profile()).unwrap();
    assert!(find(&msg, "OBX").iter().all(|o| !o.contains("|TX|")));
}

#[test]
fn nte_carries_watermark_and_the_mandatory_sentence() {
    let msg = encode(&identified(), &profile()).unwrap();
    let ntes = find(&msg, "NTE");
    assert_eq!(ntes.len(), 2);
    let wm = watermark(Mode::Identified);
    assert_eq!(ntes[0], format!("NTE|1|L|{}", escape(&wm.line())));
    assert_eq!(ntes[1], format!("NTE|2|L|{MANDATORY_SENTENCE}"));
    assert!(unescape(ntes[0]).contains("exp-0001"));
}

#[test]
fn embed_pdf_adds_one_ed_obx_that_decodes_to_the_input() {
    use base64::Engine as _;
    let pdf: Vec<u8> = (0..=255u8).chain(0..=255u8).collect();
    let mut p = profile();
    p.embed_pdf = true;
    let msg = encode_with(&identified(), &p, Some(&pdf)).unwrap();
    let ed: Vec<&str> = find(&msg, "OBX")
        .into_iter()
        .filter(|o| o.contains("|ED|"))
        .collect();
    assert_eq!(ed.len(), 1);
    let value = ed[0].split('|').nth(5).unwrap();
    let data = value.strip_prefix("^AP^PDF^Base64^").expect(value);
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(data)
        .unwrap();
    assert_eq!(decoded, pdf);
    assert!(matches!(
        encode_with(&identified(), &p, None).unwrap_err(),
        ExportError::Hl7(Hl7Error::PdfMissing)
    ));
    // Not asked for: bytes are ignored.
    let msg = encode_with(&identified(), &profile(), Some(&pdf)).unwrap();
    assert!(!msg.contains("|ED|"));
}

#[test]
fn identical_input_gives_identical_bytes() {
    let mut r = identified();
    r.review = Some(review(ReviewState::Final, false));
    assert_eq!(
        encode(&r, &profile()).unwrap(),
        encode(&r, &profile()).unwrap()
    );
}

#[test]
fn file_name_is_prefix_export_id_and_session_hash() {
    let n = file_name("asd_", "exp-0001", "session-1");
    assert!(n.starts_with("asd_exp-0001_") && n.ends_with(".hl7"), "{n}");
    assert_eq!(n.len(), "asd_exp-0001_".len() + 16 + ".hl7".len());
    assert_ne!(n, file_name("asd_", "exp-0001", "session-2"));
    assert_eq!(file_name("../x", "a/b", "s").matches('/').count(), 0);
}

// ---- escaping ------------------------------------------------------------------------------

#[test]
fn escape_table() {
    for (raw, esc) in [
        ("|", "\\F\\"),
        ("^", "\\S\\"),
        ("&", "\\T\\"),
        ("~", "\\R\\"),
        ("\\", "\\E\\"),
        ("a\r\nb", "a\\.br\\b"),
        ("a\nb", "a\\.br\\b"),
        ("नेपाल परीक्षण", "नेपाल परीक्षण"),
    ] {
        assert_eq!(escape(raw), esc);
    }
}

/// Small deterministic generator (no proptest in the workspace).
struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0 >> 33
    }
}

fn hostile(rng: &mut Lcg) -> String {
    const ALPHABET: &[&str] = &[
        "|",
        "^",
        "~",
        "\\",
        "&",
        "\r",
        "\n",
        "\r\n",
        "\0",
        "\u{b}",
        "\u{1c}",
        "a",
        "Z",
        " ",
        "नेपाल",
        "\\E\\",
        "\\.br\\",
        "\\F\\",
        "MSH|",
        "\u{2028}",
    ];
    let n = rng.next() % 24;
    (0..n)
        .map(|_| ALPHABET[(rng.next() as usize) % ALPHABET.len()])
        .collect()
}

#[test]
fn hostile_text_cannot_break_segments() {
    let mut rng = Lcg(42);
    for _ in 0..300 {
        let name = hostile(&mut rng);
        let mut r = identified();
        r.patient.mrn = Some(Mrn {
            value: hostile(&mut rng),
            assigning_authority: hostile(&mut rng),
        });
        r.patient.participant_token = hostile(&mut rng);
        r.org.name = hostile(&mut rng);
        let mut review = review(ReviewState::Final, false);
        review.signatures[0].display_name = Some(name.clone());
        review.signatures[0].user_id = Some(hostile(&mut rng));
        review.impression = Some(hostile(&mut rng));
        r.review = Some(review);
        let mut p = profile();
        p.sending_app = hostile(&mut rng);
        p.receiving_facility = hostile(&mut rng);
        p.pid5_mode = Pid5Mode::Token;
        p.send_pv1 = true;
        let mut wm = watermark(Mode::Identified);
        wm.requested_by_display = hostile(&mut rng);
        let bytes = encode_oru_r01(
            &r,
            &Hl7Options {
                profile: &p,
                delivery_id: &hostile(&mut rng),
                generated_at: GENERATED_AT,
                watermark: &wm,
                pdf: None,
            },
        )
        .unwrap();
        let msg = String::from_utf8(bytes).unwrap();

        // Only CR terminates segments; there is no LF and no MLLP framing byte.
        assert!(!msg.contains('\n') && !msg.contains('\0'));
        assert!(!msg.contains('\u{b}') && !msg.contains('\u{1c}'));
        let segs = segments(&msg);
        let names: Vec<&str> = segs
            .iter()
            .map(|s| &s[..s.find('|').unwrap_or(s.len())])
            .collect();
        // Exactly the expected segment sequence: hostile data created no extra segment.
        assert_eq!(&names[..5], ["MSH", "PID", "PV1", "ORC", "OBR"], "{msg:?}");
        assert!(
            names[5..].iter().all(|n| matches!(*n, "NTE" | "OBX")),
            "{names:?}"
        );
        assert_eq!(names.iter().filter(|n| **n == "MSH").count(), 1);
        // Field counts of structured segments are unchanged by hostile content.
        assert_eq!(segs[0].split('|').count(), 18, "MSH {:?}", segs[0]);
        assert_eq!(segs[2], "PV1|1|O");
        // The signer name round-trips through escaping.
        let obr = segs[4];
        let signer = obr.split('|').nth(32).unwrap();
        let sub: Vec<&str> = signer.split('&').collect();
        assert_eq!(sub.len(), 2, "NDL.1 has exactly id and name: {signer:?}");
        // Independent expectation: CR LF / CR / LF are one line break each, the other
        // control characters become spaces, everything else is unchanged.
        let expected = name
            .replace("\r\n", "\n")
            .replace('\r', "\n")
            .replace(['\0', '\u{b}', '\u{1c}'], " ");
        assert_eq!(unescape(sub[1]), expected, "name {name:?}");
    }
}
