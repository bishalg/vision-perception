//! Case package (SHR-1..6, XC-4, XC-6, XC-8; WP-I.12).

mod export_support;
mod support;

use std::collections::{BTreeMap, BTreeSet};
use std::io::Read as _;

use asd_contract::schema;
use asd_export::case_package::{
    self, paths, AttachmentKind, CaseAttachment, CaseConsent, CaseEncounter, CasePackageInput,
    CasePackageManifest, CaseSession, CaseSubject, EncounterKind, EncounterStatus,
};
use asd_export::deid;
use asd_export::fhir::clinical::FhirProfile;
use asd_export::fhir::{self, FhirDevice};
use asd_export::model::{ExportRecord, ReviewState};
use asd_export::policy::{Mode, Purpose};
use asd_export::watermark::{Watermark, MANDATORY_SENTENCE};
use asd_export::{claims, ExportError};
use export_support::{
    full_record, golden_ingest, later_session, sentinel, DEVICE_ID, PARTICIPANT_ID,
};
use serde_json::Value;
use sha2::{Digest, Sha256};

const EXPORT_ID: &str = "3c9d7e1a-5b2f-4a6c-8d0e-7f1a2b3c4d5e";
const SESSION_2: &str = "9a8b7c6d-5e4f-4a3b-8c2d-1e0f9a8b7c6d";
const SESSION_3: &str = "1f2e3d4c-5b6a-4978-8695-a4b3c2d1e0f9";
const SALT: [u8; deid::SALT_LEN] = [42; deid::SALT_LEN];

const RECIPIENT: &str = "Dr X, Boston RECIPIENT-SENTINEL";
const CONSENT_REF: &str = "CONSENT-FORM-SENTINEL-17";
const ENC_SUMMARY: &str = "ENCOUNTER-SUMMARY-SENTINEL text";
const ENC_REASON: &str = "ENCOUNTER-REASON-SENTINEL";
const ENC_CLINICIAN: &str = "Dr. ENCOUNTER-CLINICIAN-SENTINEL";
const ATTACHMENT_BYTES: &[u8] = b"%PDF-1.4 ATTACHMENT-SENTINEL bytes";
const ATTACHMENT_ID: &str = "6b1c2d3e-4f5a-4b6c-9d7e-8f9a0b1c2d3e";

// ---- fixtures ------------------------------------------------------------------------------

fn case_session(record: ExportRecord) -> CaseSession {
    let mut gaze = golden_ingest().raw_payload;
    gaze.session_id = record.session.summary.id.clone();
    CaseSession {
        record,
        gaze,
        device: FhirDevice::new(fhir::DEFAULT_DEVICE_NAME, "1.0"),
    }
}

/// Three sessions of one participant: signed, unsigned (no review), signed.
fn records() -> Vec<ExportRecord> {
    let first = full_record();
    let mut second = later_session(10, SESSION_2);
    second.review = None;
    let third = later_session(40, SESSION_3);
    vec![first, second, third]
}

fn watermark(mode: Mode, purpose: Purpose) -> Watermark {
    Watermark {
        export_id: EXPORT_ID.into(),
        requested_by_display: "Dr. Requester".into(),
        requested_at_utc: "2026-11-02T06:15:00Z".into(),
        mode,
        purpose,
        org_name: "Kathmandu Child Clinic".into(),
    }
}

fn encounter() -> CaseEncounter {
    CaseEncounter {
        id: "e0e1e2e3-0000-4000-8000-000000000001".into(),
        kind: EncounterKind::FollowUpVisit,
        occurred_at: "2026-10-03T09:00:00Z".into(),
        status: EncounterStatus::Open,
        clinician_display: Some(ENC_CLINICIAN.into()),
        reason: Some(ENC_REASON.into()),
        summary: Some(ENC_SUMMARY.into()),
        follow_up_due_at: Some("2026-12-01T09:00:00Z".into()),
        follow_up_done_at: None,
        session_ids: vec![SESSION_2.into()],
    }
}

fn input(mode: Mode, purpose: Purpose) -> CasePackageInput {
    CasePackageInput {
        watermark: watermark(mode, purpose),
        created_at: "2026-11-02T06:16:00Z".into(),
        deid_salt: SALT,
        recipient: RECIPIENT.into(),
        consent: Some(CaseConsent {
            reference: CONSENT_REF.into(),
            recorded_at: "2026-10-01T08:00:00Z".into(),
            recorded_by_display: "Front Desk".into(),
        }),
        sessions: records().into_iter().map(case_session).collect(),
        encounters: vec![encounter()],
        attachments: vec![CaseAttachment {
            id: ATTACHMENT_ID.into(),
            kind: AttachmentKind::Referral,
            file_name: "referral form.pdf".into(),
            media_type: "application/pdf".into(),
            bytes: ATTACHMENT_BYTES.to_vec(),
        }],
        fhir_profile: FhirProfile::default(),
        subject: None,
    }
}

/// The case without sessions: one encounter and one attachment, the participant and org
/// named in `subject` (taken from the first session's record).
fn sessionless(mode: Mode, purpose: Purpose) -> CasePackageInput {
    let mut input = input(mode, purpose);
    let first = input.sessions[0].record.clone();
    input.sessions.clear();
    input.encounters[0].session_ids.clear();
    input.subject = Some(CaseSubject {
        org_id: first.session.org_id.clone(),
        org: first.org.clone(),
        patient: first.patient.clone(),
        consent: first.consent,
    });
    input
}

fn identified() -> CasePackageInput {
    input(Mode::Identified, Purpose::ClinicalCare)
}

fn deidentified() -> CasePackageInput {
    input(Mode::Deidentified, Purpose::Research)
}

// ---- ZIP reading ---------------------------------------------------------------------------

struct Unzipped {
    names: Vec<String>,
    files: BTreeMap<String, Vec<u8>>,
}

fn unzip(bytes: &[u8]) -> Unzipped {
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).expect("a ZIP");
    let mut names = Vec::new();
    let mut files = BTreeMap::new();
    for i in 0..archive.len() {
        let mut f = archive.by_index(i).unwrap();
        let name = f.name().unwrap().to_string();
        assert!(!f.is_dir(), "no directory entries: {name}");
        assert_eq!(
            f.last_modified(),
            Some(zip::DateTime::default()),
            "fixed 1980-01-01 timestamp: {name}"
        );
        assert_eq!(f.compression(), zip::CompressionMethod::Deflated);
        assert!(
            f.extra_data().as_deref().is_none_or(<[u8]>::is_empty),
            "no extra fields: {name}"
        );
        let mut data = Vec::new();
        f.read_to_end(&mut data).unwrap();
        names.push(name.clone());
        files.insert(name, data);
    }
    Unzipped { names, files }
}

fn manifest(z: &Unzipped) -> CasePackageManifest {
    serde_json::from_slice(&z.files[paths::MANIFEST]).expect("manifest decodes")
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn attachment_path() -> String {
    format!("attachments/{ATTACHMENT_ID}-referral_form.pdf")
}

fn expected_entries(mode: Mode) -> Vec<String> {
    let mut out = vec![paths::MANIFEST, paths::README, paths::FHIR_BUNDLE];
    if cfg!(feature = "pdf") {
        out.push(paths::CASE_SUMMARY_PDF);
    }
    out.extend([paths::SESSIONS, paths::CLIPS]);
    let mut out: Vec<String> = out.into_iter().map(str::to_owned).collect();
    if mode == Mode::Identified {
        out.push(paths::NOTES.to_owned());
        out.push(attachment_path());
    }
    out
}

// ---- determinism and layout (SHR-1, XC-8) --------------------------------------------------

#[test]
fn identical_input_gives_identical_bytes() {
    for make in [identified, deidentified] {
        let a = case_package::build(&make()).unwrap();
        let b = case_package::build(&make()).unwrap();
        assert_eq!(a, b);

        // The session order of the input does not matter (job retries read rows in any
        // order and must produce the same exports.sha256).
        let mut shuffled = make();
        shuffled.sessions.reverse();
        assert_eq!(case_package::build(&shuffled).unwrap(), a);
    }
}

#[test]
fn entries_are_exactly_shr1_in_order() {
    for mode in [Mode::Identified, Mode::Deidentified] {
        let input = if mode == Mode::Identified {
            identified()
        } else {
            deidentified()
        };
        let z = unzip(&case_package::build(&input).unwrap());
        assert_eq!(z.names, expected_entries(mode), "{mode:?}");
    }
}

#[test]
fn identified_without_attachments_has_no_attachment_entries() {
    let mut input = identified();
    input.attachments.clear();
    let z = unzip(&case_package::build(&input).unwrap());
    assert!(z.names.iter().all(|n| !n.starts_with(paths::ATTACHMENTS)));
    assert!(z.names.contains(&paths::NOTES.to_owned()));
}

// ---- manifest (SHR-2) ----------------------------------------------------------------------

#[test]
fn manifest_validates_against_its_schema_and_the_committed_file() {
    let committed: Value = serde_json::from_str(
        &std::fs::read_to_string(
            support::repo_root().join("docs/spec/schemas/asd/case-package-v1.json"),
        )
        .expect("committed case-package-v1.json"),
    )
    .unwrap();
    assert_eq!(
        committed,
        case_package::manifest_schema(),
        "run `asd-cli schema export`"
    );

    for input in [identified(), deidentified()] {
        let z = unzip(&case_package::build(&input).unwrap());
        let value: Value = serde_json::from_slice(&z.files[paths::MANIFEST]).unwrap();
        let mut strict = case_package::manifest_schema();
        schema::forbid_additional_properties(&mut strict);
        if let Err(errors) = schema::validate(&strict, &value) {
            panic!("manifest invalid: {errors:#?}");
        }
    }

    // The schema rejects what the contract forbids.
    let z = unzip(&case_package::build(&deidentified()).unwrap());
    let mut bad: Value = serde_json::from_slice(&z.files[paths::MANIFEST]).unwrap();
    bad["not_diagnostic"] = Value::Bool(false);
    assert!(schema::validate(&case_package::manifest_schema(), &bad).is_err());
    let mut bad: Value = serde_json::from_slice(&z.files[paths::MANIFEST]).unwrap();
    bad["schema"] = Value::String("case-package/2.0".into());
    assert!(schema::validate(&case_package::manifest_schema(), &bad).is_err());
}

#[test]
fn every_file_hash_matches_and_the_readme_lists_them() {
    for input in [identified(), deidentified()] {
        let (returned, bytes) = case_package::build_with_manifest(&input).unwrap();
        let z = unzip(&bytes);
        let m = manifest(&z);
        assert_eq!(m, returned);
        let listed: Vec<&str> = m.files.iter().map(|f| f.path.as_str()).collect();
        let others: Vec<&str> = z.names[1..].iter().map(String::as_str).collect();
        assert_eq!(listed, others, "files[] = every other entry, in order");
        for f in &m.files {
            let data = &z.files[&f.path];
            assert_eq!(f.bytes, data.len() as u64, "{}", f.path);
            assert_eq!(f.sha256, hex(&Sha256::digest(data)), "{}", f.path);
        }

        // README: the `sha256sum -c` lines are exactly the entries after the README.
        let readme = String::from_utf8(z.files[paths::README].clone()).unwrap();
        let begin = readme.find("BEGIN SHA256SUMS -----\n").unwrap() + 23;
        let end = readme.find("----- END SHA256SUMS").unwrap();
        let sums: Vec<(String, String)> = readme[begin..end]
            .lines()
            .map(|l| {
                let (h, p) = l.split_once("  ").unwrap();
                (h.to_owned(), p.to_owned())
            })
            .collect();
        let want: Vec<(String, String)> = m.files[1..]
            .iter()
            .map(|f| (f.sha256.clone(), f.path.clone()))
            .collect();
        assert_eq!(sums, want);
        for (h, p) in &sums {
            assert_eq!(*h, hex(&Sha256::digest(&z.files[p])));
        }

        assert_eq!(m.schema, case_package::SCHEMA_TAG);
        assert_eq!(m.package_id, EXPORT_ID);
        assert_eq!(m.sessions_n, 3);
        assert!(m.not_diagnostic);
        assert_eq!(m.watermark, input.watermark.line());
        assert!(readme.contains(MANDATORY_SENTENCE));
        assert!(readme.contains(claims::MISSION));
        assert!(readme.contains(EXPORT_ID));
        assert!(readme.contains("controller"));
        assert!(readme.contains("नेपाली"));
    }
}

#[test]
fn manifest_fields_follow_the_mode() {
    let z = unzip(&case_package::build(&identified()).unwrap());
    let m: Value = serde_json::from_slice(&z.files[paths::MANIFEST]).unwrap();
    assert_eq!(m["mode"], "identified");
    assert_eq!(m["purpose"], "clinical_care");
    assert_eq!(m["recipient"]["label"], RECIPIENT);
    assert_eq!(m["consent"]["reference"], CONSENT_REF);
    assert_eq!(m["participant"]["mrn"]["value"], sentinel::MRN);
    assert_eq!(m["participant"]["mrn"]["authority"], sentinel::AUTHORITY);
    assert_eq!(
        m["participant"]["token"],
        full_record().patient.participant_token
    );
    assert_eq!(m["date_range"]["from"].as_str().unwrap().len(), 10);

    let z = unzip(&case_package::build(&deidentified()).unwrap());
    let m: Value = serde_json::from_slice(&z.files[paths::MANIFEST]).unwrap();
    assert_eq!(m["mode"], "deidentified");
    assert_eq!(m["recipient"]["label"], "de-identified recipient");
    assert!(m.get("consent").is_none());
    assert!(m["participant"]["pseudonym"]
        .as_str()
        .unwrap()
        .starts_with("P-"));
    assert!(m["participant"].get("token").is_none());
    for k in ["from", "to"] {
        let d = m["date_range"][k].as_str().unwrap();
        assert_eq!(d.len(), 7, "YYYY-MM: {d}");
    }
}

#[test]
fn identified_mrn_only_when_given() {
    let mut input = identified();
    for s in &mut input.sessions {
        s.record.patient.mrn = None;
    }
    let bytes = case_package::build(&input).unwrap();
    let z = unzip(&bytes);
    let m: Value = serde_json::from_slice(&z.files[paths::MANIFEST]).unwrap();
    assert!(m["participant"].get("mrn").is_none());
    assert!(!contains(&bytes_of(&z), sentinel::MRN.as_bytes()));
}

// ---- de-identification (SHR-3) -------------------------------------------------------------

fn bytes_of(z: &Unzipped) -> Vec<u8> {
    let mut all = Vec::new();
    for (name, data) in &z.files {
        all.extend_from_slice(name.as_bytes());
        all.push(0);
        all.extend_from_slice(data);
        all.push(0);
    }
    all
}

fn contains(hay: &[u8], needle: &[u8]) -> bool {
    hay.windows(needle.len()).any(|w| w == needle)
}

#[test]
fn deidentified_archive_holds_none_of_the_forbidden_strings() {
    let input = deidentified();
    let raw = case_package::build(&input).unwrap();
    let z = unzip(&raw);
    let all = bytes_of(&z);
    let token = full_record().patient.participant_token;
    let session_1 = full_record().session.summary.id;
    let mut forbidden: Vec<String> = vec![
        token,
        PARTICIPANT_ID.into(),
        session_1,
        SESSION_2.into(),
        SESSION_3.into(),
        sentinel::MRN.into(),
        sentinel::SIGNER.into(),
        sentinel::COSIGNER.into(),
        sentinel::REGISTRATION.into(),
        sentinel::SIGNER_ID.into(),
        "cosigner-id-sentinel".into(),
        sentinel::NOTE.into(),
        sentinel::NOTE_AUTHOR.into(),
        sentinel::IMPRESSION.into(),
        sentinel::RECOMMENDATIONS.into(),
        sentinel::LETTER_BODY.into(),
        sentinel::LETTER_RECIPIENT.into(),
        sentinel::CONTENT_HASH.into(),
        DEVICE_ID.into(),
        RECIPIENT.into(),
        CONSENT_REF.into(),
        ENC_SUMMARY.into(),
        ENC_REASON.into(),
        ENC_CLINICIAN.into(),
        ATTACHMENT_ID.into(),
        "ATTACHMENT-SENTINEL".into(),
        hex(&SALT),
    ];
    // Exact session start times (minute precision) never appear: dates are months.
    for r in records() {
        forbidden.push(r.session.summary.started_at[..16].to_owned());
    }
    for f in &forbidden {
        assert!(!contains(&all, f.as_bytes()), "found {f:?} in the archive");
        assert!(
            !contains(&raw, f.as_bytes()),
            "found {f:?} in the ZIP bytes"
        );
    }
    assert!(!z
        .names
        .iter()
        .any(|n| n.starts_with("notes/") || n.starts_with("attachments/")));

    // Control: the identified package of the same case does carry these values, so the
    // search above is not vacuous.
    let identified_all = bytes_of(&unzip(&case_package::build(&identified()).unwrap()));
    for f in [
        full_record().patient.participant_token.as_str(),
        SESSION_2,
        SESSION_3,
        sentinel::MRN,
        sentinel::SIGNER,
        sentinel::NOTE,
        DEVICE_ID,
        ENC_SUMMARY,
        "ATTACHMENT-SENTINEL",
    ] {
        assert!(contains(&identified_all, f.as_bytes()), "control: {f:?}");
    }
}

fn pseudonyms(text: &str, prefix: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let mut rest = text;
    while let Some(i) = rest.find(prefix) {
        let tail = &rest[i..];
        let p: String = tail.chars().take(prefix.len() + 10).collect();
        if p.len() == prefix.len() + 10
            && p[prefix.len()..]
                .chars()
                .all(|c| c.is_ascii_uppercase() || ('2'..='7').contains(&c))
        {
            out.insert(p);
        }
        rest = &rest[i + prefix.len()..];
    }
    out
}

#[test]
fn pseudonyms_match_across_the_three_data_files() {
    let z = unzip(&case_package::build(&deidentified()).unwrap());
    let fhir = String::from_utf8(z.files[paths::FHIR_BUNDLE].clone()).unwrap();
    let jsonl = String::from_utf8(z.files[paths::SESSIONS].clone()).unwrap();
    let csv = String::from_utf8(z.files[paths::CLIPS].clone()).unwrap();

    let bundle: Value = serde_json::from_str(&fhir).unwrap();
    let patient_ids: Vec<&str> = entries_of(&bundle, "Patient")
        .iter()
        .map(|p| p["id"].as_str().unwrap())
        .collect();
    assert_eq!(patient_ids.len(), 1);

    let mut jsonl_p = BTreeSet::new();
    let mut jsonl_s = BTreeSet::new();
    for line in jsonl.lines().skip(1) {
        let v: Value = serde_json::from_str(line).unwrap();
        jsonl_p.insert(v["participant_id"].as_str().unwrap().to_owned());
        jsonl_s.insert(v["session_id"].as_str().unwrap().to_owned());
    }
    let csv_s: BTreeSet<String> = csv
        .lines()
        .skip(1)
        .map(|l| l.split(',').nth(1).unwrap().to_owned())
        .collect();

    assert_eq!(jsonl_s.len(), 3);
    assert_eq!(jsonl_p, BTreeSet::from([patient_ids[0].to_owned()]));
    assert_eq!(pseudonyms(&fhir, "P-"), jsonl_p);
    assert_eq!(pseudonyms(&fhir, "S-"), jsonl_s);
    assert_eq!(csv_s, jsonl_s);

    let m = manifest(&z);
    let case_package::manifest::ParticipantInfo::Deidentified { pseudonym } = m.participant else {
        panic!("de-identified participant");
    };
    assert_eq!(BTreeSet::from([pseudonym]), jsonl_p);

    // One salt per package: another salt, other pseudonyms.
    let mut other = deidentified();
    other.deid_salt = [1; deid::SALT_LEN];
    let z2 = unzip(&case_package::build(&other).unwrap());
    let fhir2 = String::from_utf8(z2.files[paths::FHIR_BUNDLE].clone()).unwrap();
    assert!(pseudonyms(&fhir2, "P-").is_disjoint(&jsonl_p));
}

// ---- identified extras ---------------------------------------------------------------------

#[test]
fn identified_archive_has_notes_attachments_and_the_mrn() {
    let z = unzip(&case_package::build(&identified()).unwrap());
    let notes = String::from_utf8(z.files[paths::NOTES].clone()).unwrap();
    for s in [
        sentinel::NOTE,
        sentinel::IMPRESSION,
        sentinel::LETTER_BODY,
        ENC_SUMMARY,
        ENC_CLINICIAN,
    ] {
        assert!(notes.contains(s), "notes.json lacks {s:?}");
    }
    assert_eq!(z.files[&attachment_path()], ATTACHMENT_BYTES);
    let manifest_text = String::from_utf8(z.files[paths::MANIFEST].clone()).unwrap();
    assert!(manifest_text.contains(sentinel::MRN));
    let jsonl = String::from_utf8(z.files[paths::SESSIONS].clone()).unwrap();
    assert_eq!(jsonl.lines().count(), 4, "manifest line + 3 sessions");
    assert!(jsonl.contains(SESSION_2));
}

// ---- FHIR (SHR-4) --------------------------------------------------------------------------

fn entries_of<'a>(bundle: &'a Value, ty: &str) -> Vec<&'a Value> {
    bundle["entry"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| &e["resource"])
        .filter(|r| r["resourceType"] == ty)
        .collect()
}

#[test]
fn fhir_is_one_collection_over_every_session() {
    let z = unzip(&case_package::build(&identified()).unwrap());
    let bundle: Value = serde_json::from_slice(&z.files[paths::FHIR_BUNDLE]).unwrap();
    assert_eq!(bundle["type"], "collection");
    assert_eq!(entries_of(&bundle, "Patient").len(), 1);
    assert_eq!(entries_of(&bundle, "Organization").len(), 1);
    assert_eq!(entries_of(&bundle, "Device").len(), 1);
    assert_eq!(
        entries_of(&bundle, "Practitioner").len(),
        2,
        "two distinct signers"
    );
    assert_eq!(entries_of(&bundle, "Observation").len(), 3);
    assert_eq!(
        entries_of(&bundle, "DiagnosticReport").len(),
        2,
        "one per signed session"
    );
    assert_eq!(
        entries_of(&bundle, "Provenance").len(),
        4,
        "two signatures x two"
    );

    // fullUrls are unique and every reference resolves inside the bundle.
    let urls: Vec<&str> = bundle["entry"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["fullUrl"].as_str().unwrap())
        .collect();
    let set: BTreeSet<&str> = urls.iter().copied().collect();
    assert_eq!(set.len(), urls.len());
    let text = bundle.to_string();
    for part in text.split("\"reference\":\"").skip(1) {
        let r = &part[..part.find('"').unwrap()];
        assert!(set.contains(r), "unresolved reference {r}");
    }

    // De-identified: no Practitioner and no Provenance.
    let z = unzip(&case_package::build(&deidentified()).unwrap());
    let bundle: Value = serde_json::from_slice(&z.files[paths::FHIR_BUNDLE]).unwrap();
    assert!(entries_of(&bundle, "Practitioner").is_empty());
    assert!(entries_of(&bundle, "Provenance").is_empty());
    assert_eq!(entries_of(&bundle, "DiagnosticReport").len(), 2);
}

// ---- PDF (SHR-5) ---------------------------------------------------------------------------

/// Text runs of each page content stream.
#[cfg(feature = "pdf")]
fn page_texts(pdf: &[u8]) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = pdf;
    while let Some(i) = find(rest, b"stream\n") {
        let body = &rest[i + 7..];
        let end = find(body, b"endstream").unwrap();
        out.push(support::pdf_text_runs(&body[..end]).join("\n"));
        rest = &body[end + 9..];
    }
    out
}

#[cfg(feature = "pdf")]
fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

#[cfg(feature = "pdf")]
#[test]
fn pdf_pages_carry_watermark_sentences_and_banners() {
    for input in [identified(), deidentified()] {
        let z = unzip(&case_package::build(&input).unwrap());
        let pdf = &z.files[paths::CASE_SUMMARY_PDF];
        let pages = page_texts(pdf);
        assert_eq!(pages.len(), support::pdf_page_count(pdf));
        assert!(pages.len() >= 5, "cover, three sessions, encounters");
        let wm = input.watermark.line();
        for (i, page) in pages.iter().enumerate() {
            assert!(page.contains(&wm), "page {i} lacks the watermark");
            assert!(page.contains(MANDATORY_SENTENCE), "page {i}");
            assert!(page.contains(claims::MISSION), "page {i}");
            assert!(page.contains(&format!("Page {}/{}", i + 1, pages.len())));
        }
        let all = pages.join("\n");
        assert_eq!(
            all.matches("PRELIMINARY - NOT SIGNED").count(),
            1,
            "only the unsigned session"
        );
        assert!(!all.to_lowercase().contains("screening"));
        assert!(!all.to_lowercase().contains("diagnosis"));
        if input.watermark.mode == Mode::Identified {
            assert!(all.contains(sentinel::SIGNER));
            assert!(all.contains(ENC_SUMMARY));
            assert!(all.contains(RECIPIENT));
        } else {
            assert!(!all.contains(sentinel::SIGNER));
            assert!(!all.contains(ENC_SUMMARY));
        }
    }
}

#[cfg(feature = "pdf")]
#[test]
fn awaiting_cosign_prints_its_own_banner() {
    let mut input = identified();
    let review = input.sessions[0].record.review.as_mut().unwrap();
    review.state = ReviewState::AwaitingCosign;
    review.signatures.truncate(1);
    let z = unzip(&case_package::build(&input).unwrap());
    let all = page_texts(&z.files[paths::CASE_SUMMARY_PDF]).join("\n");
    assert!(all.contains("AWAITING SECOND SIGNATURE"));
}

#[cfg(not(feature = "pdf"))]
#[test]
fn without_the_pdf_feature_the_manifest_marks_the_pdf_absent() {
    let z = unzip(&case_package::build(&identified()).unwrap());
    let m = manifest(&z);
    assert!(!z.files.contains_key(paths::CASE_SUMMARY_PDF));
    assert_eq!(m.absent.len(), 1);
    assert_eq!(m.absent[0].path, paths::CASE_SUMMARY_PDF);
    assert!(m.files.iter().all(|f| f.path != paths::CASE_SUMMARY_PDF));
    let _ = ReviewState::Final;
}

// ---- refusals ------------------------------------------------------------------------------

fn refused(input: &CasePackageInput) -> ExportError {
    case_package::build(input).expect_err("refused")
}

#[test]
fn inconsistent_input_is_refused() {
    let mut no_consent = identified();
    no_consent.consent = None;
    assert!(matches!(refused(&no_consent), ExportError::CasePackage(_)));

    let mut two_people = identified();
    two_people.sessions[1].record.patient.participant_id = "someone-else".into();
    assert!(matches!(refused(&two_people), ExportError::CasePackage(_)));

    let mut empty = deidentified();
    empty.sessions.clear();
    assert!(matches!(refused(&empty), ExportError::CasePackage(_)));

    // Research is de-identified only (XC-5).
    let research_identified = input(Mode::Identified, Purpose::Research);
    assert!(matches!(
        refused(&research_identified),
        ExportError::Policy(_)
    ));

    // Research needs research consent on every session.
    let mut no_research_consent = deidentified();
    no_research_consent.sessions[2]
        .record
        .consent
        .research_consented = None;
    assert!(matches!(
        refused(&no_research_consent),
        ExportError::CasePackage(_)
    ));

    // De-identified packages need no consent reference (policy decides).
    let mut deid_no_consent = deidentified();
    deid_no_consent.consent = None;
    assert!(case_package::build(&deid_no_consent).is_ok());
}

// ---- a case without sessions (visits and documents only) -----------------------------------

#[test]
fn a_case_without_sessions_builds_a_valid_package() {
    for mode in [Mode::Identified, Mode::Deidentified] {
        let input = sessionless(mode, Purpose::ClinicalCare);
        let (m, bytes) = case_package::build_with_manifest(&input).unwrap();
        assert_eq!(bytes, case_package::build(&input).unwrap(), "deterministic");
        let z = unzip(&bytes);
        assert_eq!(z.names, expected_entries(mode), "{mode:?}");

        let value: Value = serde_json::from_slice(&z.files[paths::MANIFEST]).unwrap();
        let mut strict = case_package::manifest_schema();
        schema::forbid_additional_properties(&mut strict);
        if let Err(errors) = schema::validate(&strict, &value) {
            panic!("manifest invalid: {errors:#?}");
        }
        assert_eq!(m.sessions_n, 0);
        for f in &m.files {
            assert_eq!(
                f.sha256,
                hex(&Sha256::digest(&z.files[&f.path])),
                "{}",
                f.path
            );
        }

        let bundle: Value = serde_json::from_slice(&z.files[paths::FHIR_BUNDLE]).unwrap();
        assert_eq!(bundle["type"], "collection");
        assert_eq!(entries_of(&bundle, "Patient").len(), 1);
        assert_eq!(entries_of(&bundle, "Organization").len(), 1);
        assert_eq!(bundle["entry"].as_array().unwrap().len(), 2, "{bundle}");
        assert!(entries_of(&bundle, "DiagnosticReport").is_empty());

        let jsonl = String::from_utf8(z.files[paths::SESSIONS].clone()).unwrap();
        assert_eq!(jsonl.lines().count(), 1, "the manifest line alone");
        let readme = String::from_utf8(z.files[paths::README].clone()).unwrap();
        assert!(readme.contains("Sessions: 0"), "{readme}");

        if mode == Mode::Identified {
            assert_eq!(m.date_range.from, "2026-10-03");
            assert!(String::from_utf8_lossy(&z.files[paths::MANIFEST]).contains(sentinel::MRN));
            assert_eq!(z.files[&attachment_path()], ATTACHMENT_BYTES);
        } else {
            assert_eq!(m.date_range.from, "2026-10");
            let all = bytes_of(&z);
            for s in [PARTICIPANT_ID, sentinel::MRN, ENC_SUMMARY, ENC_REASON] {
                assert!(!contains(&all, s.as_bytes()), "{s:?} leaked");
            }
            let pseudonym = deid::pseudonym(&SALT, "P-", PARTICIPANT_ID);
            assert_eq!(entries_of(&bundle, "Patient")[0]["id"], pseudonym.as_str());
        }
    }

    // Attachments only, identified: still a package; the date range is the package date.
    let mut only_files = sessionless(Mode::Identified, Purpose::ClinicalCare);
    only_files.encounters.clear();
    let (m, _) = case_package::build_with_manifest(&only_files).unwrap();
    assert_eq!(m.date_range.from, m.date_range.to);
}

#[test]
fn a_case_without_sessions_needs_a_subject_and_something_to_share() {
    let mut no_subject = sessionless(Mode::Identified, Purpose::ClinicalCare);
    no_subject.subject = None;
    assert!(matches!(refused(&no_subject), ExportError::CasePackage(_)));

    let mut nothing = sessionless(Mode::Identified, Purpose::ClinicalCare);
    nothing.encounters.clear();
    nothing.attachments.clear();
    assert!(matches!(refused(&nothing), ExportError::CasePackage(_)));

    // De-identified packages never carry attachments: those alone are nothing to share.
    let mut deid_files = sessionless(Mode::Deidentified, Purpose::ClinicalCare);
    deid_files.encounters.clear();
    assert!(matches!(refused(&deid_files), ExportError::CasePackage(_)));

    // Research needs research consent, which a case without sessions does not carry.
    let mut research = sessionless(Mode::Deidentified, Purpose::Research);
    research
        .subject
        .as_mut()
        .unwrap()
        .consent
        .research_consented = None;
    assert!(matches!(refused(&research), ExportError::CasePackage(_)));

    // A subject naming someone else than the sessions is refused.
    let mut mismatch = identified();
    mismatch.subject = sessionless(Mode::Identified, Purpose::ClinicalCare).subject;
    mismatch.subject.as_mut().unwrap().patient.participant_id = "someone-else".into();
    assert!(matches!(refused(&mismatch), ExportError::CasePackage(_)));
}
