//! owner: WP-I.12
//!
//! `fhir/bundle.json` of a case package (SHR-4): the clinical report bundle encoder
//! ([`crate::fhir::clinical`], FHX-*) run once per session and merged into one
//! `collection` Bundle. Nothing is re-implemented: every resource is the one the
//! per-session encoder builds, with its deterministic v5 id.
//!
//! Entry order: one `Patient`, one `Organization`, one `Practitioner` per signer
//! (identified only), one `Device` per recording app version, then per session in date
//! order its gaze `Observation`, its `DiagnosticReport` when a signed review exists, and a
//! `Provenance` per signature (identified only). A case without sessions holds the `Patient`
//! and `Organization` only. Shared resources are de-duplicated by
//! `fullUrl` (same id = same resource). Unverified codes stay dropped (XC-7), the
//! de-identified rules of the clinical encoder apply unchanged (XC-4).

use std::collections::BTreeSet;

use serde_json::{json, Value};

use super::Case;
use crate::fhir::clinical::{
    clinical_report_bundle, subject_entries, BundleKind, ClinicalBundleInput,
};
use crate::ids::uuid5;
use crate::ExportError;

/// Shared resource types, in entry order.
const SHARED: [&str; 4] = ["Patient", "Organization", "Practitioner", "Device"];

/// The merged bundle as a JSON value.
pub(crate) fn case_bundle(case: &Case<'_>) -> Result<Value, ExportError> {
    let wm = &case.input.watermark;
    let mut shared: [Vec<Value>; 4] = Default::default();
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut per_session: Vec<Value> = Vec::new();
    let mut head: Option<(Value, Value, Value)> = None; // meta, identifier, timestamp

    for (session, record) in case.sessions.iter().zip(&case.records) {
        let bundle = clinical_report_bundle(&ClinicalBundleInput {
            record,
            gaze: &session.gaze,
            device: session.device.clone(),
            watermark: wm,
            profile: &case.input.fhir_profile,
            document: None,
            kind: BundleKind::Collection,
        })?;
        let signed = record.review.as_ref().is_some_and(|r| r.is_signed());
        head.get_or_insert_with(|| {
            (
                bundle["meta"].clone(),
                bundle["identifier"].clone(),
                bundle["timestamp"].clone(),
            )
        });
        for entry in bundle["entry"].as_array().into_iter().flatten() {
            let ty = entry["resource"]["resourceType"]
                .as_str()
                .unwrap_or_default();
            if let Some(i) = SHARED.iter().position(|s| *s == ty) {
                let url = entry["fullUrl"].as_str().unwrap_or_default().to_owned();
                if seen.insert(url) {
                    shared[i].push(entry.clone());
                }
            } else if ty == "DiagnosticReport" && !signed {
                // SHR-4: a DiagnosticReport only where a signed review exists.
            } else {
                per_session.push(entry.clone());
            }
        }
    }

    let (meta, identifier, timestamp) = match head {
        Some(head) => head,
        None => {
            // No session: the Patient and Organization alone, no DiagnosticReport.
            let (meta, identifier, timestamp, subject) = subject_entries(
                &case.participant,
                &case.org_id,
                &case.org.name,
                wm,
                &case.input.fhir_profile,
            )?;
            shared[0].extend(subject);
            (meta, identifier, timestamp)
        }
    };
    let entries: Vec<Value> = shared.into_iter().flatten().chain(per_session).collect();
    Ok(json!({
        "resourceType": "Bundle",
        "id": uuid5(&format!("Bundle/case/{}", wm.export_id)),
        "meta": meta,
        "identifier": identifier,
        "type": "collection",
        "timestamp": timestamp,
        "entry": entries,
    }))
}

/// Compact JSON bytes (XC-8).
pub(crate) fn encode(case: &Case<'_>) -> Result<Vec<u8>, ExportError> {
    Ok(serde_json::to_vec(&case_bundle(case)?).expect("JSON values serialise"))
}
