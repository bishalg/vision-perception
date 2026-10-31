//! owner: WP-B.8 (moved from `asd-report` into `asd-export` by WP-I.1, XC-2)
//!
//! FHIR R4 export unit: a `Bundle` of `type: collection` (ADR-0003 decision 1).
//!
//! Port of the iOS `FHIRBundleBuilder` + `FHIRInteroperabilityMapper`; the iOS golden
//! `Tests/Fixtures/expected-bundle.json` is the parity oracle (`tests/fhir_parity.rs`).
//!
//! Entries, in order: `Patient`, `RelatedPerson`, `Device`, the gaze `Observation` (omitted
//! when [`crate::observation_status`] is `None`: simulated, synthetic or rejected, SIM-5 /
//! FHIR-2) and its `Provenance`, the M-CHAT-R/F `QuestionnaireResponse` and score
//! `Observation` (each with a `Provenance`), then the report `DocumentReference` when a PDF
//! is given. Every `fullUrl` is a name-based (v5) `urn:uuid`, so the same input always
//! yields the same bundle, and every reference resolves inside the bundle.
//!
//! JSON objects come out key-sorted (serde_json without `preserve_order`), as the iOS
//! encoder's `.sortedKeys`.

use std::collections::BTreeMap;

use asd_contract::session_v1_6::GazeSessionRecord;
use asd_core::types::CaptureSource;
use base64::Engine;
use serde_json::{json, Map, Value};

use crate::ids::{fhir_timestamp, urn, uuid5, LOCAL};
use crate::mchat::{MchatResult, QUESTION_CODES};
use crate::{observation_status, ReportError};

/// `FHIRDeviceInfo.defaultDisplayName`.
pub const DEFAULT_DEVICE_NAME: &str = "EarlyGaze Observation Diary";
/// Media type of a bundle response.
pub const CONTENT_TYPE: &str = "application/fhir+json";

/// Report title on the `DocumentReference` (iOS `toFHIRDocumentReference`).
const DOCUMENT_TITLE: &str = "EarlyGaze Observation Diary report";

// ---- namespaces (FHIRNamespaces, ADR-0003 decision 7) -----------------------------------

const LOINC: &str = "http://loinc.org";
const SNOMED: &str = "http://snomed.info/sct";
const UCUM: &str = "http://unitsofmeasure.org";
const OBSERVATION_CATEGORY: &str = "http://terminology.hl7.org/CodeSystem/observation-category";
const DATA_OPERATION: &str = "http://terminology.hl7.org/CodeSystem/v3-DataOperation";
const PROVENANCE_PARTICIPANT_TYPE: &str =
    "http://terminology.hl7.org/CodeSystem/provenance-participant-type";
const ROLE_CODE: &str = "http://terminology.hl7.org/CodeSystem/v3-RoleCode";
const MCHAT_QUESTIONNAIRE: &str = "http://loinc.org/q/80628-1";
const US_CORE_DOC_CATEGORY: &str =
    "http://hl7.org/fhir/us/core/CodeSystem/us-core-documentreference-category";
const IHE_FORMAT: &str = "http://ihe.net/fhir/ValueSet/IHE.FormatCode.codesystem";

fn local(path: &str) -> String {
    format!("{LOCAL}/{path}")
}

fn coding(system: &str, code: &str, display: &str) -> Value {
    json!({"system": system, "code": code, "display": display})
}

fn reference(url: &str) -> Value {
    json!({"reference": url})
}

// ---- inputs --------------------------------------------------------------------------------

/// Who the bundle is about: a pseudonymous token only (CNS-7). `Patient` carries no name
/// and no birth date.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FhirSubject {
    /// `Patient.id`.
    pub participant_token: String,
    /// Exported as the age-in-months extension instead of a birth date.
    pub age_months: Option<i64>,
    /// `RelatedPerson.name`, only when the caregiver typed one and chose to include it.
    pub caregiver_display_name: Option<String>,
}

impl FhirSubject {
    /// A blank caregiver name (after trimming) is treated as absent.
    pub fn new(token: &str, age_months: Option<i64>, caregiver: Option<&str>) -> Self {
        Self {
            participant_token: token.to_owned(),
            age_months,
            caregiver_display_name: caregiver
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_owned),
        }
    }
}

/// The recording app, the `Device` every `Provenance` names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FhirDevice {
    pub display_name: String,
    /// The app's marketing version.
    pub version: String,
}

impl FhirDevice {
    pub fn new(display_name: &str, version: &str) -> Self {
        Self {
            display_name: display_name.to_owned(),
            version: version.to_owned(),
        }
    }
}

/// The rendered observation diary, embedded as a `DocumentReference` attachment.
#[derive(Debug, Clone, Copy)]
pub struct ReportDocument<'a> {
    pub pdf: &'a [u8],
}

/// Everything one bundle is built from.
pub struct BundleInput<'a> {
    pub record: &'a GazeSessionRecord,
    pub subject: FhirSubject,
    pub device: FhirDevice,
    /// The questionnaire result and its answers (question id 1..=20 -> answer).
    pub mchat: Option<(&'a MchatResult, &'a BTreeMap<u8, bool>)>,
    pub document: Option<ReportDocument<'a>>,
}

// ---- builder -------------------------------------------------------------------------------

/// `urn:uuid` of the bundle's `Patient`, for callers adding resources about it.
pub fn patient_reference(subject: &FhirSubject) -> String {
    urn(&uuid5(&format!("Patient/{}", subject.participant_token)))
}

struct Entries {
    items: Vec<Value>,
    device_url: String,
    person_url: String,
}

impl Entries {
    fn push(&mut self, url: &str, resource: Value) {
        self.items
            .push(json!({"fullUrl": url, "resource": resource}));
    }

    fn push_provenance(&mut self, target: &str, recorded: &str, gaze: Option<&GazeSessionRecord>) {
        let id = uuid5(&format!(
            "Provenance/{}",
            target.trim_start_matches("urn:uuid:")
        ));
        let resource = provenance(
            &id,
            target,
            recorded,
            &self.device_url,
            &self.person_url,
            gaze,
        );
        self.push(&urn(&id), resource);
    }
}

/// Builds the bundle. Fails only on a stored date that is not RFC 3339.
pub fn bundle(input: &BundleInput<'_>) -> Result<Value, ReportError> {
    let record = input.record;
    let token = &input.subject.participant_token;
    let patient_url = patient_reference(&input.subject);
    let person_id = uuid5(&format!("RelatedPerson/{token}"));
    let device_id = uuid5(&format!(
        "Device/{}/{}",
        input.device.display_name, input.device.version
    ));
    let mut entries = Entries {
        items: Vec::new(),
        device_url: urn(&device_id),
        person_url: urn(&person_id),
    };

    entries.push(&patient_url, patient(&input.subject));
    let person = related_person(&person_id, &input.subject, &patient_url);
    entries.push(&urn(&person_id), person);
    entries.push(&urn(&device_id), device(&device_id, &input.device));

    if let Some(status) = observation_status(record) {
        let gaze_id = uuid5(&format!("Observation/gaze/{}", record.session_id));
        let obs = gaze_observation(record, &gaze_id, status, &patient_url, &entries.device_url)?;
        entries.push(&urn(&gaze_id), obs);
        let recorded = fhir_timestamp("endedAt", &record.ended_at)?;
        entries.push_provenance(&urn(&gaze_id), &recorded, Some(record));
    }

    if let Some((result, answers)) = input.mchat {
        let authored = fhir_timestamp("completionDate", &result.completion_date)?;
        let response_id = uuid5(&format!("QuestionnaireResponse/mchat/{token}/{authored}"));
        let score_id = uuid5(&format!("Observation/mchat-score/{token}/{authored}"));
        let response = questionnaire_response(
            answers,
            &response_id,
            &authored,
            &patient_url,
            &entries.person_url,
        );
        entries.push(&urn(&response_id), response);
        entries.push_provenance(&urn(&response_id), &authored, None);
        let score = mchat_score(
            result,
            &score_id,
            &authored,
            &patient_url,
            &urn(&response_id),
        );
        entries.push(&urn(&score_id), score);
        entries.push_provenance(&urn(&score_id), &authored, None);
    }

    if let Some(doc) = input.document {
        let doc_id = uuid5(&format!("DocumentReference/report/{}", record.session_id));
        let date = fhir_timestamp("startedAt", &record.started_at)?;
        entries.push(
            &urn(&doc_id),
            document_reference(&doc_id, doc.pdf, &date, &patient_url),
        );
    }

    Ok(json!({
        "resourceType": "Bundle",
        "id": uuid5(&format!("Bundle/{token}/{}", record.session_id)),
        "type": "collection",
        "entry": entries.items,
    }))
}

// ---- resources -----------------------------------------------------------------------------

fn patient(subject: &FhirSubject) -> Value {
    let mut p = json!({
        "resourceType": "Patient",
        "id": subject.participant_token,
        "identifier": [{
            "system": local("NamingSystem/participant-token"),
            "value": subject.participant_token,
        }],
    });
    if let Some(months) = subject.age_months {
        p["extension"] = json!([{
            "url": local("StructureDefinition/age-in-months"),
            "valueQuantity": {"value": months, "unit": "mo", "system": UCUM, "code": "mo"},
        }]);
    }
    p
}

fn related_person(id: &str, subject: &FhirSubject, patient_url: &str) -> Value {
    let mut p = json!({
        "resourceType": "RelatedPerson",
        "id": id,
        "patient": reference(patient_url),
        "relationship": [{"coding": [coding(ROLE_CODE, "PRN", "parent")]}],
    });
    if let Some(name) = &subject.caregiver_display_name {
        p["name"] = json!([{"text": name}]);
    }
    p
}

fn device(id: &str, device: &FhirDevice) -> Value {
    json!({
        "resourceType": "Device",
        "id": id,
        "deviceName": [{"name": device.display_name, "type": "user-friendly-name"}],
        "version": [{"value": device.version}],
        "type": {"text": "Mobile gaze engagement observation app"},
    })
}

fn gaze_observation(
    record: &GazeSessionRecord,
    id: &str,
    status: &str,
    patient_url: &str,
    device_url: &str,
) -> Result<Value, ReportError> {
    let mut obs = json!({
        "resourceType": "Observation",
        "id": id,
        "identifier": [{"system": local("NamingSystem/gaze-session"), "value": record.session_id}],
        "status": status,
        "category": [{"coding": [coding(OBSERVATION_CATEGORY, "exam", "Exam")]}],
        "code": {
            "coding": [
                coding(LOINC, "93044-6", "Gaze tracking assessment panel"),
                coding(SNOMED, "713444003", "Eye tracking examination (procedure)"),
            ],
            "text": "Gaze engagement observation (parent-run phone app)",
        },
        "subject": reference(patient_url),
        "effectiveDateTime": fhir_timestamp("startedAt", &record.started_at)?,
        "component": gaze_components(record),
        "device": reference(device_url),
    });
    // SIM-4: records written before provenance existed say so.
    if record.legacy_provenance == Some(true) {
        obs["meta"] = json!({"tag": [coding(
            &local("CodeSystem/record-provenance"),
            "legacy-provenance",
            "Capture source inferred from a record written before provenance was recorded",
        )]});
    }
    Ok(obs)
}

/// Components in a fixed order; unmeasured metrics are absent (PLC-3). First-look latency
/// is never exported: there is no real first-fixation detector yet (PLC-1).
fn gaze_components(record: &GazeSessionRecord) -> Vec<Value> {
    let system = local("CodeSystem/gaze-engagement");
    let mut out = Vec::new();
    let mut add = |code: &str, display: &str, text: &str, value: Option<f64>, unit: &str| {
        let Some(value) = value else { return };
        out.push(json!({
            "code": {"coding": [coding(&system, code, display)], "text": text},
            "valueQuantity": {"value": value, "unit": unit, "system": UCUM, "code": "1"},
        }));
    };
    let r = record;
    add(
        "social-preference-score",
        "Social visual preference score",
        "Social Visual Preference Score",
        Some(r.social_preference_score),
        "score",
    );
    add(
        "eye-region-dwell-ratio",
        "Eye region dwell ratio",
        "Eye Region Dwell Ratio",
        r.eye_dwell_ratio,
        "ratio",
    );
    add(
        "mouth-region-dwell-ratio",
        "Mouth region dwell ratio",
        "Mouth Region Dwell Ratio",
        r.mouth_dwell_ratio,
        "ratio",
    );
    add(
        "valid-frame-ratio",
        "Valid frame ratio",
        "Valid Frame Quality Ratio",
        Some(r.valid_frame_ratio),
        "ratio",
    );
    add(
        "social-smile-intensity-mean",
        "Social smile intensity mean",
        "Social Smile Intensity Mean",
        Some(r.social_smile_intensity),
        "score",
    );
    add(
        "smile-reactivity-index",
        "Smile reactivity index",
        "Smile Reactivity Index",
        Some(r.smile_reactivity_index),
        "score",
    );
    add(
        "flat-affect-score",
        "Neutral expression score",
        "Neutral Expression Score",
        Some(r.flat_affect_score),
        "score",
    );
    add(
        "surprise-score-mean",
        "Surprise score mean",
        "Surprise Score Mean",
        Some(r.surprise_score_mean),
        "score",
    );
    add(
        "composite-engagement",
        "Per-stimulus composite engagement score",
        "Per-Stimulus Composite Engagement Score",
        r.per_video_composite,
        "score",
    );
    for video in r.per_video.iter().flatten().filter(|v| v.has_baseline) {
        add(
            &format!("clip-score-{}", video.stimulus_id),
            &video.title,
            &format!("Stimulus subscore: {}", video.title),
            Some(video.subscore),
            "score",
        );
    }
    out
}

fn questionnaire_response(
    answers: &BTreeMap<u8, bool>,
    id: &str,
    authored: &str,
    patient_url: &str,
    author_url: &str,
) -> Value {
    let items: Vec<Value> = (1u8..=20)
        .zip(QUESTION_CODES)
        .map(|(q, code)| {
            let mut item = json!({"linkId": code, "text": format!("M-CHAT-R/F item {q}")});
            if let Some(answer) = answers.get(&q) {
                item["answer"] = json!([{"valueBoolean": answer}]);
            }
            item
        })
        .collect();
    json!({
        "resourceType": "QuestionnaireResponse",
        "id": id,
        "status": "completed",
        "questionnaire": MCHAT_QUESTIONNAIRE,
        "subject": reference(patient_url),
        "authored": authored,
        "item": items,
        "author": reference(author_url),
        "source": reference(author_url),
    })
}

/// Total score (`80649-7`) with the category (`80650-5`) as a component (FHIR-4).
fn mchat_score(
    result: &MchatResult,
    id: &str,
    effective: &str,
    patient_url: &str,
    response_url: &str,
) -> Value {
    json!({
        "resourceType": "Observation",
        "id": id,
        "status": "final",
        "category": [{"coding": [coding(OBSERVATION_CATEGORY, "survey", "Survey")]}],
        "code": {
            "coding": [coding(LOINC, "80649-7", "M-CHAT-R total score [Score]")],
            "text": "M-CHAT-R/F total score",
        },
        "subject": reference(patient_url),
        "effectiveDateTime": effective,
        "valueInteger": result.total_failed,
        "derivedFrom": [reference(response_url)],
        "component": [{
            "code": {"coding": [coding(LOINC, "80650-5", "M-CHAT-R risk level [Interpretation]")]},
            "valueCodeableConcept": {"text": result.risk_level.category_text()},
        }],
    })
}

/// Agent = the app (`Device`) acting for the caregiver; activity CREATE. For a gaze session
/// it also names the capture source (SIM-4) and the on-device vault file it came from.
fn provenance(
    id: &str,
    target: &str,
    recorded: &str,
    device_url: &str,
    person_url: &str,
    gaze: Option<&GazeSessionRecord>,
) -> Value {
    let mut p = json!({
        "resourceType": "Provenance",
        "id": id,
        "target": [reference(target)],
        "recorded": recorded,
        "activity": {"coding": [coding(DATA_OPERATION, "CREATE", "create")]},
        "agent": [{
            "type": {"coding": [coding(PROVENANCE_PARTICIPANT_TYPE, "device", "Device")]},
            "who": reference(device_url),
            "onBehalfOf": reference(person_url),
        }],
    });
    let Some(record) = gaze else { return p };
    let (code, display) = capture_source(record.capture_source);
    let mut extensions = vec![json!({
        "url": local("StructureDefinition/capture-source"),
        "valueCoding": coding(&local("CodeSystem/capture-source"), code, display),
    })];
    if record.legacy_provenance == Some(true) {
        extensions.push(json!({
            "url": local("StructureDefinition/capture-source-inferred"),
            "valueBoolean": true,
        }));
    }
    p["extension"] = Value::Array(extensions);
    p["entity"] = json!([{
        "role": "source",
        "what": {
            "identifier": {
                "system": local("NamingSystem/session-vault"),
                "value": format!("session_{}", record.session_id),
            },
            "display": "On-device encrypted session vault file",
        },
    }]);
    p
}

fn capture_source(source: CaptureSource) -> (&'static str, &'static str) {
    match source {
        CaptureSource::TrueDepth => ("truedepth", "Apple TrueDepth front camera (live capture)"),
        CaptureSource::Simulated => ("simulated", "Simulated data"),
        CaptureSource::Synthetic => ("synthetic", "Synthetic data"),
    }
}

/// The rendered report (iOS `toFHIRDocumentReference`) with a deterministic id, a bundle
/// subject reference, `size` and `hash`, and no empty `author` array (FHIR forbids `[]`).
fn document_reference(id: &str, pdf: &[u8], date: &str, patient_url: &str) -> Value {
    let engine = base64::engine::general_purpose::STANDARD;
    let hash = engine.encode(sha1_smol::Sha1::from(pdf).digest().bytes());
    let mut attachment = Map::new();
    attachment.insert("contentType".into(), json!("application/pdf"));
    attachment.insert("language".into(), json!("en"));
    attachment.insert("data".into(), json!(engine.encode(pdf)));
    attachment.insert("size".into(), json!(pdf.len()));
    attachment.insert("hash".into(), json!(hash));
    attachment.insert("title".into(), json!(DOCUMENT_TITLE));
    json!({
        "resourceType": "DocumentReference",
        "id": id,
        "status": "current",
        "type": {"coding": [coding(LOINC, "34108-1", "Outpatient note")]},
        "category": [{"coding": [coding(US_CORE_DOC_CATEGORY, "clinical-note", "Clinical Note")]}],
        "subject": {"reference": patient_url, "display": "Child (pseudonymous)"},
        "date": date,
        "description": DOCUMENT_TITLE,
        "content": [{
            "attachment": attachment,
            "format": coding(IHE_FORMAT, "urn:ihe:iti:xfdf:2008", "PDF Document"),
        }],
        "context": {"event": [{"coding": [coding(
            SNOMED, "713444003", "Eye tracking examination (procedure)",
        )]}]},
    })
}
