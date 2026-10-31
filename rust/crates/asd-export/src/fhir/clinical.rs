//! owner: WP-I.3
//!
//! The FHIR R4 clinical report bundle (FHX-1..8), built from one
//! [`crate::model::ExportRecord`] plus the stored session record (`sessions.summary`).
//!
//! # Relation to the session bundle (FHX-1)
//!
//! [`super::session`] is the WP-B.8 port of the iOS `FHIRBundleBuilder`, served by
//! `GET /v1/sessions/{id}/fhir` and byte-checked against the iOS fixture. This module does
//! not fork it: it calls [`super::session::bundle`] and takes the gaze `Observation` and the
//! `Device` from its output, so the same session gives the same resources and the same v5
//! ids (`http://curv.org/fhir/<name>`) on the device, in the session bundle and here. The
//! session bundle itself is unchanged.
//!
//! Two differences to the parity Observation, both forced by rules the session bundle
//! predates:
//!
//! - **XC-7.** Codings whose [`crate::codes`] entry is unverified are dropped from the
//!   Observation unless the profile opts in ([`FhirProfile::codes`]). Today that removes
//!   LOINC `93044-6` and SNOMED CT `713444003` from `Observation.code` (both refuted by the
//!   terminology server, see `codes.rs`), leaving `code.text`. With the opt-in the
//!   Observation is byte-identical to the parity one after JCS (tested).
//! - **XC-4.** In de-identified mode the Observation is built for the pseudonymous session
//!   id (so its id is not the v5 hash of the real one) and `effectiveDateTime` is the
//!   `YYYY-MM` month the de-identified record carries.
//!
//! # Contents (FHX-2), in entry order
//!
//! `Patient`, `Organization`, `Practitioner` per signer (identified only), `Device`, the
//! gaze `Observation` (absent for a rejected-quality session), `DiagnosticReport`,
//! `DocumentReference` (when a PDF is given) and `Provenance` per signature (identified
//! only: de-identified signature times are month-precision and `Provenance.recorded` is an
//! instant). Simulated and synthetic sessions are refused (SIM-5).
//!
//! `Bundle.type` is `collection` for downloads and `transaction` for push (WP-I.9): resources
//! with a stable business identifier are written conditionally (`POST` + `ifNoneExist` for
//! Patient, Organization and Practitioner; conditional `PUT` for the Observation, report
//! and document, which an amendment updates); Device and Provenance, which have none, are
//! `PUT` to their deterministic ids.
//!
//! # De-identification
//!
//! The caller passes records through [`crate::deid::apply`] before any encoder sees them
//! (as for every format). This encoder refuses a de-identified export whose record did not
//! go through it ([`ExportError::NotDeidentified`]), and in that mode never emits a
//! Practitioner, Provenance, MRN, `conclusion` or `issued`, whatever the input holds.

use asd_contract::session_v1_6::GazeSessionRecord;
use base64::Engine;
use serde_json::{json, Map, Value};

use super::session::FhirSubject;
use super::session::{bundle as session_bundle, patient_reference, BundleInput, FhirDevice};
use crate::claims;
use crate::codes::{self, Code, CodePolicy, UnverifiedCode};
use crate::ids::{fhir_timestamp, urn, uuid5, LOCAL};
use crate::model::{Disposition, ExportRecord, ReportStatus, Signature, SignatureRole};
use crate::policy::Mode;
use crate::watermark::{Watermark, MANDATORY_SENTENCE};
use crate::{export_refusal, ExportError, ReportError};

/// RFC 4122 v5 (SHA-1) UUID in the URL namespace for `http://curv.org/fhir/<name>`,
/// lower-case: the iOS `FHIRBundleBuilder.uuid` scheme every bundle id uses (trap 14).
pub fn resource_id(name: &str) -> String {
    uuid5(name)
}

/// Download (`collection`) or push (`transaction`, WP-I.9).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BundleKind {
    Collection,
    Transaction,
}

/// What the attached PDF is; selects `DocumentReference.type` (FHX-5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocumentKind {
    /// The PDF/A clinical summary (PDF-6).
    ClinicalSummary,
    /// A referral letter (NOTE-3).
    Referral,
}

impl DocumentKind {
    fn code(self) -> &'static Code {
        match self {
            Self::ClinicalSummary => &codes::LOINC_SUMMARY_OF_EPISODE_NOTE,
            Self::Referral => &codes::LOINC_REFERRAL_NOTE,
        }
    }

    fn title(self) -> &'static str {
        match self {
            Self::ClinicalSummary => "Clinical summary of a gaze engagement observation",
            Self::Referral => "Referral letter",
        }
    }
}

/// The rendered PDF behind the `DocumentReference`.
#[derive(Debug, Clone, Copy)]
pub struct ClinicalDocument<'a> {
    pub kind: DocumentKind,
    pub pdf: &'a [u8],
    /// Where the receiver can fetch the PDF (push). When set, the attachment carries `url`
    /// instead of inline `data`; `size` and `hash` are always present.
    pub url: Option<&'a str>,
}

/// The FHIR part of the org's integration profile (INTG-3 `fhir {mrn_system,
/// registration_system}`) and its code opt-in (XC-7).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FhirProfile {
    /// `Patient.identifier.system` of the MRN. Unset: a local system under
    /// `http://curv.org/fhir/NamingSystem/mrn/<assigning authority>`.
    pub mrn_system: Option<String>,
    /// `Practitioner.identifier.system` of the professional registration. Unset: the
    /// registration is not sent (FHX-6).
    pub registration_system: Option<String>,
    pub codes: CodePolicy,
}

/// Everything one clinical bundle is built from.
pub struct ClinicalBundleInput<'a> {
    /// The export record, already de-identified when `watermark.mode` is de-identified.
    pub record: &'a ExportRecord,
    /// The stored session (`sessions.summary`, identified) the gaze Observation comes from.
    pub gaze: &'a GazeSessionRecord,
    /// The recording app (the session bundle's `Device`).
    pub device: FhirDevice,
    /// Export id, mode and purpose (FHX-7) and the narrative watermark (XC-6).
    pub watermark: &'a Watermark,
    pub profile: &'a FhirProfile,
    pub document: Option<ClinicalDocument<'a>>,
    pub kind: BundleKind,
}

/// The bundle as compact JSON bytes (XC-1, XC-8: identical input, identical bytes).
pub fn encode(input: &ClinicalBundleInput<'_>) -> Result<Vec<u8>, ExportError> {
    let bundle = clinical_report_bundle(input)?;
    Ok(serde_json::to_vec(&bundle).expect("JSON values serialise"))
}

// ---- local URIs ----------------------------------------------------------------------------

fn local(path: &str) -> String {
    format!("{LOCAL}/{path}")
}

const NS_TOKEN: &str = "NamingSystem/participant-token";
const NS_PSEUDONYM: &str = "NamingSystem/export-pseudonym";
const NS_MRN: &str = "NamingSystem/mrn";
const NS_CONSOLE_USER: &str = "NamingSystem/console-user";
const NS_ORGANIZATION: &str = "NamingSystem/organization";
const NS_EXPORT: &str = "NamingSystem/export";
const NS_GAZE_SESSION: &str = "NamingSystem/gaze-session";
const NS_REVIEW: &str = "NamingSystem/gaze-session-review";
const CS_GAZE: &str = "CodeSystem/gaze-engagement";
const CS_DISPOSITION: &str = "CodeSystem/clinical-disposition";
const CS_EXPORT_MODE: &str = "CodeSystem/export-mode";

fn coding(code: &Code) -> Value {
    json!({"system": code.system, "code": code.code, "display": code.display})
}

fn local_coding(system: &str, code: &str, display: &str) -> Value {
    json!({"system": local(system), "code": code, "display": display})
}

fn reference(url: &str) -> Value {
    json!({"reference": url})
}

fn identifier(system: &str, value: &str) -> Value {
    json!({"system": system, "value": value})
}

// ---- builder -------------------------------------------------------------------------------

struct Entry {
    full_url: String,
    resource: Value,
    request: Value,
}

/// Builds the bundle (FHX-1..7). Fails on a simulated or synthetic session (SIM-5), a
/// timestamp that is not RFC 3339, an unverified code without the opt-in (XC-7), or a
/// de-identified export of a record that skipped [`crate::deid::apply`] (XC-4).
pub fn clinical_report_bundle(input: &ClinicalBundleInput<'_>) -> Result<Value, ExportError> {
    let record = input.record;
    let mode = input.watermark.mode;
    let deidentified = mode == Mode::Deidentified;
    let policy = input.profile.codes;

    if let Some(refusal) = export_refusal(input.gaze) {
        if refusal.is_capture_source() {
            return Err(ReportError::Refused(refusal).into());
        }
    }
    if deidentified && (record.days_since_first_session.is_none() || record.patient.mrn.is_some()) {
        return Err(ExportError::NotDeidentified);
    }

    let session_id = record.session.summary.id.as_str();
    let token = record.patient.participant_token.as_str();
    let status = record.report_status();
    let mut entries: Vec<Entry> = Vec::new();

    // Observation and Device from the session bundle (FHX-1). De-identified: built for the
    // pseudonymous ids so nothing hashes back to the real session.
    let mut gaze = input.gaze.clone();
    if deidentified {
        gaze.session_id = session_id.to_owned();
        gaze.participant_id = token.to_owned();
    }
    let subject = FhirSubject::new(token, None, None);
    let parity = session_bundle(&BundleInput {
        record: &gaze,
        subject: subject.clone(),
        device: input.device.clone(),
        mchat: None,
        document: None,
    })?;
    let take = |ty: &str| -> Option<(String, Value)> {
        parity["entry"].as_array()?.iter().find_map(|e| {
            (e["resource"]["resourceType"] == ty).then(|| {
                (
                    e["fullUrl"].as_str().unwrap_or_default().to_owned(),
                    e["resource"].clone(),
                )
            })
        })
    };
    let (device_url, device) = take("Device").expect("the session bundle always has a Device");
    let observation = take("Observation")
        .map(|(url, mut obs)| -> Result<(String, Value), ExportError> {
            gate_codings(&mut obs, policy)?;
            if deidentified {
                obs["effectiveDateTime"] = json!(record.session.summary.started_at);
            }
            Ok((url, obs))
        })
        .transpose()?;

    // Patient (FHX-3).
    let patient_url = patient_reference(&subject);
    let (patient, patient_key) = patient(record, deidentified, input.profile, policy)?;
    entries.push(Entry {
        full_url: patient_url.clone(),
        resource: patient,
        request: conditional_create("Patient", &patient_key),
    });

    // Organization.
    let org_id = record.session.org_id.as_str();
    let org_uuid = uuid5(&format!("Organization/{org_id}"));
    let org_url = urn(&org_uuid);
    let org_key = (local(NS_ORGANIZATION), org_id.to_owned());
    entries.push(Entry {
        full_url: org_url.clone(),
        resource: json!({
            "resourceType": "Organization",
            "id": org_uuid,
            "identifier": [identifier(&org_key.0, &org_key.1)],
            "name": record.org.name,
        }),
        request: conditional_create("Organization", &org_key),
    });

    // Practitioners (FHX-6): identified only, one per distinct signer.
    let signatures: &[Signature] = record
        .review
        .as_ref()
        .map_or(&[], |r| r.signatures.as_slice());
    let mut practitioners: Vec<(String, String)> = Vec::new(); // (user id, fullUrl)
    if !deidentified {
        for sig in signatures {
            let Some(user_id) = sig.user_id.as_deref() else {
                continue;
            };
            if practitioners.iter().any(|(u, _)| u == user_id) {
                continue;
            }
            let id = uuid5(&format!("Practitioner/{user_id}"));
            let url = urn(&id);
            let key = (local(NS_CONSOLE_USER), user_id.to_owned());
            let mut ids = vec![identifier(&key.0, &key.1)];
            if let (Some(system), Some(reg)) = (
                input.profile.registration_system.as_deref(),
                sig.professional_registration.as_deref(),
            ) {
                ids.push(identifier(system, reg));
            }
            let mut p = json!({"resourceType": "Practitioner", "id": id, "identifier": ids});
            if let Some(name) = sig.display_name.as_deref() {
                p["name"] = json!([{"text": name}]);
            }
            entries.push(Entry {
                full_url: url.clone(),
                resource: p,
                request: conditional_create("Practitioner", &key),
            });
            practitioners.push((user_id.to_owned(), url));
        }
    }

    // Device.
    let device_id = device["id"].as_str().unwrap_or_default().to_owned();
    entries.push(Entry {
        full_url: device_url,
        resource: device,
        request: put(&format!("Device/{device_id}")),
    });

    // Observation.
    let observation_url = observation.as_ref().map(|(url, _)| url.clone());
    if let Some((url, obs)) = observation {
        let key = (local(NS_GAZE_SESSION), session_id.to_owned());
        entries.push(Entry {
            full_url: url,
            resource: obs,
            request: conditional_update("Observation", &key),
        });
    }

    // DiagnosticReport (FHX-4).
    let report_uuid = uuid5(&format!("DiagnosticReport/{session_id}"));
    let report_url = urn(&report_uuid);
    let report_key = (local(NS_REVIEW), session_id.to_owned());
    let report = diagnostic_report(&ReportParts {
        id: &report_uuid,
        key: &report_key,
        record,
        status,
        deidentified,
        patient_url: &patient_url,
        org_url: &org_url,
        observation_url: observation_url.as_deref(),
        interpreters: &practitioners,
        watermark: input.watermark,
    })?;
    entries.push(Entry {
        full_url: report_url.clone(),
        resource: report,
        request: conditional_update("DiagnosticReport", &report_key),
    });

    // DocumentReference (FHX-5).
    if let Some(doc) = input.document {
        let export_id = input.watermark.export_id.as_str();
        let id = uuid5(&format!("DocumentReference/{export_id}"));
        let key = (local(NS_EXPORT), export_id.to_owned());
        let resource = document_reference(&DocumentParts {
            id: &id,
            key: &key,
            doc,
            status,
            date: &fhir_timestamp(
                "watermark.requested_at_utc",
                &input.watermark.requested_at_utc,
            )?,
            patient_url: &patient_url,
            org_url: &org_url,
            report_url: &report_url,
            policy,
        })?;
        entries.push(Entry {
            full_url: urn(&id),
            resource,
            request: conditional_update("DocumentReference", &key),
        });
    }

    // Provenance per signature (identified only).
    if !deidentified {
        if let Some(review) = record.review.as_ref() {
            for (i, sig) in review.signatures.iter().enumerate() {
                let Some((_, who)) = sig
                    .user_id
                    .as_deref()
                    .and_then(|u| practitioners.iter().find(|(id, _)| id == u))
                else {
                    continue;
                };
                let id = uuid5(&format!(
                    "Provenance/signature/{session_id}/{}/{i}",
                    review.version_no
                ));
                let agent_type = match sig.role {
                    SignatureRole::Sign => &codes::PROVENANCE_PARTICIPANT_AUTHOR,
                    SignatureRole::Cosign => &codes::PROVENANCE_PARTICIPANT_VERIFIER,
                }
                .require(policy)?;
                entries.push(Entry {
                    full_url: urn(&id),
                    resource: json!({
                        "resourceType": "Provenance",
                        "id": id,
                        "target": [reference(&report_url)],
                        "recorded": fhir_timestamp("signature.signed_at", &sig.signed_at)?,
                        "agent": [{
                            "type": {"coding": [coding(agent_type)]},
                            "who": reference(who),
                            "onBehalfOf": reference(&org_url),
                        }],
                    }),
                    request: put(&format!("Provenance/{id}")),
                });
            }
        }
    }

    // Bundle (FHX-7 watermark).
    let wm = input.watermark;
    let transaction = input.kind == BundleKind::Transaction;
    let items: Vec<Value> = entries
        .into_iter()
        .map(|e| {
            let mut item = json!({"fullUrl": e.full_url, "resource": e.resource});
            if transaction {
                item["request"] = e.request;
            }
            item
        })
        .collect();
    Ok(json!({
        "resourceType": "Bundle",
        "id": uuid5(&format!("Bundle/clinical/{}/{session_id}", wm.export_id)),
        "meta": {"tag": [
            local_coding(CS_EXPORT_MODE, mode_code(mode), mode.label()),
            local_coding(CS_EXPORT_MODE, purpose_code(wm), wm.purpose.label()),
        ]},
        "identifier": identifier(&local(NS_EXPORT), &wm.export_id),
        "type": if transaction { "transaction" } else { "collection" },
        "timestamp": fhir_timestamp("watermark.requested_at_utc", &wm.requested_at_utc)?,
        "entry": items,
    }))
}

fn mode_code(mode: Mode) -> &'static str {
    match mode {
        Mode::Identified => "identified",
        Mode::Deidentified => "deidentified",
    }
}

/// The wire name of the purpose (EXJ-1), the serde name of [`crate::policy::Purpose`].
fn purpose_code(wm: &Watermark) -> &'static str {
    use crate::policy::Purpose;
    match wm.purpose {
        Purpose::ClinicalCare => "clinical_care",
        Purpose::Research => "research",
        Purpose::QualityImprovement => "quality_improvement",
        Purpose::PatientRequest => "patient_request",
    }
}

// ---- XC-7 gate for reused resources --------------------------------------------------------

/// Drops every external coding the policy does not allow from `coding` arrays (an emptied
/// array is removed: FHIR forbids `[]`); any other external `system`+`code` pair (a UCUM
/// `Quantity`) must be allowed or the build fails. Local codes pass. An external code that
/// is not in the table at all is treated as unverified.
fn gate_codings(v: &mut Value, policy: CodePolicy) -> Result<(), UnverifiedCode> {
    let allowed = |system: &str, code: &str| -> Result<(), UnverifiedCode> {
        if system.starts_with(LOCAL) {
            return Ok(());
        }
        match codes::lookup(system, code) {
            Some(entry) => entry.require(policy).map(|_| ()),
            None if policy.allow_unverified => Ok(()),
            None => Err(UnverifiedCode {
                system: "(not in codes.rs)",
                code: "(not in codes.rs)",
            }),
        }
    };
    match v {
        Value::Object(map) => {
            if let Some(Value::Array(list)) = map.get_mut("coding") {
                list.retain(|c| {
                    let system = c["system"].as_str().unwrap_or_default();
                    let code = c["code"].as_str().unwrap_or_default();
                    allowed(system, code).is_ok()
                });
                if list.is_empty() {
                    map.remove("coding");
                }
            }
            if let (Some(Value::String(system)), Some(Value::String(code))) =
                (map.get("system"), map.get("code"))
            {
                allowed(system, code)?;
            }
            for child in map.values_mut() {
                gate_codings(child, policy)?;
            }
        }
        Value::Array(items) => {
            for child in items {
                gate_codings(child, policy)?;
            }
        }
        _ => {}
    }
    Ok(())
}

// ---- resources -----------------------------------------------------------------------------

/// FHX-3. Returns the resource and the identifier used for conditional create (the MRN
/// when present, else the token or pseudonym).
fn patient(
    record: &ExportRecord,
    deidentified: bool,
    profile: &FhirProfile,
    policy: CodePolicy,
) -> Result<(Value, (String, String)), ExportError> {
    let token = record.patient.participant_token.as_str();
    let mut ids = Vec::new();
    let mut key = None;
    if deidentified {
        let k = (local(NS_PSEUDONYM), token.to_owned());
        ids.push(identifier(&k.0, &k.1));
        key = Some(k);
    } else {
        if let Some(mrn) = record.patient.mrn.as_ref() {
            let system = profile
                .mrn_system
                .clone()
                .unwrap_or_else(|| local(&format!("{NS_MRN}/{}", mrn.assigning_authority)));
            let mr = codes::V2_IDENTIFIER_TYPE_MR.require(policy)?;
            ids.push(json!({
                "type": {"coding": [coding(mr)]},
                "system": system,
                "value": mrn.value,
                "assigner": {"display": mrn.assigning_authority},
            }));
            key = Some((system, mrn.value.clone()));
        }
        let k = (local(NS_TOKEN), token.to_owned());
        ids.push(identifier(&k.0, &k.1));
        key.get_or_insert(k);
    }
    let resource = json!({"resourceType": "Patient", "id": token, "identifier": ids});
    Ok((resource, key.expect("at least one identifier")))
}

fn disposition_display(d: Disposition) -> &'static str {
    match d {
        Disposition::NoConcernNoted => "No concern noted",
        Disposition::Monitor => "Monitor",
        Disposition::ReferForEvaluation => "Refer for evaluation",
        Disposition::RecaptureRequested => "Recapture requested",
        Disposition::NotInterpretable => "Not interpretable",
    }
}

struct ReportParts<'a> {
    id: &'a str,
    key: &'a (String, String),
    record: &'a ExportRecord,
    status: ReportStatus,
    deidentified: bool,
    patient_url: &'a str,
    org_url: &'a str,
    observation_url: Option<&'a str>,
    interpreters: &'a [(String, String)],
    watermark: &'a Watermark,
}

/// FHX-4.
fn diagnostic_report(p: &ReportParts<'_>) -> Result<Value, ExportError> {
    let review = p.record.review.as_ref();
    let disposition = review.and_then(|r| r.disposition);
    let effective = if p.deidentified {
        p.record.session.summary.started_at.clone()
    } else {
        fhir_timestamp("session.started_at", &p.record.session.summary.started_at)?
    };

    let mut paragraphs = vec![format!(
        "Clinician review of a gaze engagement observation. Report status: {}.",
        p.status.as_str()
    )];
    if let Some(d) = disposition {
        paragraphs.push(format!(
            "Clinician's statement (disposition): {}.",
            disposition_display(d)
        ));
    }
    paragraphs.push(p.watermark.line());
    paragraphs.push(MANDATORY_SENTENCE.to_owned());
    paragraphs.push(claims::MISSION.to_owned());
    let div = format!(
        "<div xmlns=\"http://www.w3.org/1999/xhtml\">{}</div>",
        paragraphs
            .iter()
            .map(|t| format!("<p>{}</p>", xhtml_escape(t)))
            .collect::<String>()
    );

    let mut r = Map::new();
    r.insert("resourceType".into(), json!("DiagnosticReport"));
    r.insert("id".into(), json!(p.id));
    r.insert("text".into(), json!({"status": "generated", "div": div}));
    r.insert("identifier".into(), json!([identifier(&p.key.0, &p.key.1)]));
    r.insert("status".into(), json!(p.status.as_str()));
    r.insert(
        "code".into(),
        json!({
            "coding": [local_coding(CS_GAZE, "gaze-observation-review", "Gaze observation review")],
            "text": "Clinician review of a gaze engagement observation",
        }),
    );
    r.insert("subject".into(), reference(p.patient_url));
    r.insert("effectiveDateTime".into(), json!(effective));
    if !p.deidentified {
        if let Some(signed_at) = review.and_then(|r| r.signed_at.as_deref()) {
            r.insert(
                "issued".into(),
                json!(fhir_timestamp("review.signed_at", signed_at)?),
            );
        }
    }
    r.insert("performer".into(), json!([reference(p.org_url)]));
    if !p.deidentified && !p.interpreters.is_empty() {
        let refs: Vec<Value> = p.interpreters.iter().map(|(_, u)| reference(u)).collect();
        r.insert("resultsInterpreter".into(), json!(refs));
    }
    if let Some(url) = p.observation_url {
        r.insert("result".into(), json!([reference(url)]));
    }
    if !p.deidentified {
        if let Some(impression) = review
            .and_then(|r| r.impression.as_deref())
            .filter(|s| !s.trim().is_empty())
        {
            r.insert("conclusion".into(), json!(impression));
        }
    }
    if let Some(d) = disposition {
        r.insert(
            "conclusionCode".into(),
            json!([{"coding": [local_coding(CS_DISPOSITION, d.as_str(), disposition_display(d))]}]),
        );
    }
    Ok(Value::Object(r))
}

struct DocumentParts<'a> {
    id: &'a str,
    key: &'a (String, String),
    doc: ClinicalDocument<'a>,
    status: ReportStatus,
    date: &'a str,
    patient_url: &'a str,
    org_url: &'a str,
    report_url: &'a str,
    policy: CodePolicy,
}

/// FHX-5. `attachment.hash` is SHA-1 (R4 `Attachment.hash`, trap 15).
fn document_reference(p: &DocumentParts<'_>) -> Result<Value, ExportError> {
    let engine = base64::engine::general_purpose::STANDARD;
    let doc_type = p.doc.kind.code().require(p.policy)?;
    let format = codes::IHE_FORMAT_MIME_TYPE_SUFFICIENT.require(p.policy)?;
    let mut attachment = Map::new();
    attachment.insert("contentType".into(), json!("application/pdf"));
    match p.doc.url {
        Some(url) => attachment.insert("url".into(), json!(url)),
        None => attachment.insert("data".into(), json!(engine.encode(p.doc.pdf))),
    };
    attachment.insert("size".into(), json!(p.doc.pdf.len()));
    attachment.insert(
        "hash".into(),
        json!(engine.encode(sha1_smol::Sha1::from(p.doc.pdf).digest().bytes())),
    );
    attachment.insert("title".into(), json!(p.doc.kind.title()));
    Ok(json!({
        "resourceType": "DocumentReference",
        "id": p.id,
        "identifier": [identifier(&p.key.0, &p.key.1)],
        "status": "current",
        "docStatus": p.status.as_str(),
        "type": {"coding": [coding(doc_type)]},
        "subject": reference(p.patient_url),
        "date": p.date,
        "author": [reference(p.org_url)],
        "description": p.doc.kind.title(),
        "content": [{"attachment": attachment, "format": coding(format)}],
        "context": {"related": [reference(p.report_url)]},
    }))
}

// ---- transaction requests ------------------------------------------------------------------

fn conditional_create(ty: &str, key: &(String, String)) -> Value {
    json!({"method": "POST", "url": ty, "ifNoneExist": identifier_query(key)})
}

fn conditional_update(ty: &str, key: &(String, String)) -> Value {
    json!({"method": "PUT", "url": format!("{ty}?{}", identifier_query(key))})
}

fn put(url: &str) -> Value {
    json!({"method": "PUT", "url": url})
}

/// `identifier=<system>|<value>` with both parts percent-encoded except unreserved
/// characters and `:` `/` (the `|` separator stays literal).
fn identifier_query((system, value): &(String, String)) -> String {
    format!("identifier={}|{}", percent(system), percent(value))
}

fn percent(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-._~:/".contains(&b) {
            out.push(char::from(b));
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

fn xhtml_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(ch),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percent_keeps_unreserved_and_uri_punctuation() {
        assert_eq!(percent("urn:oid:1.2/a_b~c"), "urn:oid:1.2/a_b~c");
        assert_eq!(percent("a b|c&d=é"), "a%20b%7Cc%26d%3D%C3%A9");
    }

    #[test]
    fn gate_drops_unverified_codings_and_keeps_text() {
        let mut v = json!({"code": {"coding": [
            {"system": codes::system::LOINC, "code": "93044-6", "display": "x"},
        ], "text": "t"}});
        gate_codings(&mut v, CodePolicy::default()).unwrap();
        assert_eq!(v, json!({"code": {"text": "t"}}));

        let mut kept = json!({"coding": [{"system": codes::system::LOINC, "code": "93044-6"}]});
        let opt_in = CodePolicy {
            allow_unverified: true,
        };
        gate_codings(&mut kept, opt_in).unwrap();
        assert_eq!(kept["coding"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn gate_refuses_an_unverified_quantity_code() {
        let mut v = json!({"valueQuantity": {"system": codes::system::UCUM, "code": "mo"}});
        assert!(gate_codings(&mut v, CodePolicy::default()).is_err());
    }
}
