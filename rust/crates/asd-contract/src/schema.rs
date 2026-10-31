//! owner: WP-R.3
//!
//! JSON Schema export of the contract types (CON-3) and drift check (GATE-1).
//!
//! `schemars` generates JSON Schema 2020-12; the output is then normalised so the files keep
//! the shape the existing consumers read (the Swift test validator in
//! `SessionIngestPayloadTests` understands `type` as a string, `anyOf`, `$ref`, `items`,
//! `required`, `properties`):
//! - a nullable `{"type": ["X", "null"], ...}` becomes `{"anyOf": [{"type": "X", ...}, {"type": "null"}]}`
//!   like the former Pydantic export;
//! - numeric `format` hints (`double`, `int64`, `uint32`, ...) are dropped; `date-time` stays;
//! - `"default": null` is dropped (no field has a server-side default, CON-14).
//!
//! Files are written pretty-printed with sorted keys and a trailing newline; `check`
//! compares after RFC 8785 canonicalisation, so formatting never counts as drift.

use std::fmt;
use std::path::{Path, PathBuf};

use schemars::generate::SchemaSettings;
use schemars::{JsonSchema, SchemaGenerator};
use serde_json::{json, Map, Value};

use crate::config::{self, ConfigBundle, ScoringProfile};
use crate::problem::{InvalidParam, Problem};
use crate::session_v1_6::{self, SessionIngestRequest, SessionIngestResponse};
use crate::session_v1_7;
use crate::signing::canonical_bytes;
use crate::{api_v1, care, clinical, export};

/// File name of the session ingest schema (1.6, the wire version of current clients).
pub const SESSION_FILE: &str = "session-v1.6.json";
/// File name of the session ingest schema 1.7 (SCH-1, WP-G.6).
pub const SESSION_V1_7_FILE: &str = "session-v1.7.json";
/// File name of the config bundle schema.
pub const CONFIG_BUNDLE_FILE: &str = "config-bundle-v1.json";
/// File name of the `/v1` DTO schema.
pub const API_FILE: &str = "api-v1.json";
/// `$id` of the `/v1` DTO schema.
pub const API_SCHEMA_ID: &str = "https://schemas.curv.org/asd/api-v1.json";

/// Keywords whose values are data, not subschemas; never rewritten.
const DATA_KEYWORDS: [&str; 4] = ["examples", "const", "enum", "default"];
/// Keywords that describe a schema rather than constrain it; they stay on the outer
/// `anyOf` when a nullable type is split.
const ANNOTATIONS: [&str; 8] = [
    "description",
    "title",
    "default",
    "examples",
    "deprecated",
    "readOnly",
    "writeOnly",
    "$comment",
];
const NUMERIC_FORMATS: [&str; 12] = [
    "double", "float", "int", "int8", "int16", "int32", "int64", "uint", "uint8", "uint16",
    "uint32", "uint64",
];

fn generator() -> SchemaGenerator {
    SchemaSettings::draft2020_12().into_generator()
}

fn normalise(value: &mut Value) {
    match value {
        Value::Object(map) => {
            // Split first so the moved keywords are normalised inside the new branch.
            split_nullable_type(map);
            for (key, child) in map.iter_mut() {
                if !DATA_KEYWORDS.contains(&key.as_str()) {
                    normalise(child);
                }
            }
            if matches!(map.get("format"), Some(Value::String(f)) if NUMERIC_FORMATS.contains(&f.as_str()))
            {
                map.remove("format");
            }
            if map.get("default") == Some(&Value::Null) {
                map.remove("default");
            }
        }
        Value::Array(items) => items.iter_mut().for_each(normalise),
        _ => {}
    }
}

fn split_nullable_type(map: &mut Map<String, Value>) {
    let Some(Value::Array(types)) = map.get("type") else {
        return;
    };
    let null = Value::String("null".into());
    if types.len() != 2 || !types.contains(&null) {
        return;
    }
    let other = types.iter().find(|t| **t != null).cloned().unwrap_or(null);
    map.remove("type");
    let mut inner = Map::new();
    inner.insert("type".into(), other);
    let moved: Vec<String> = map
        .keys()
        .filter(|k| !ANNOTATIONS.contains(&k.as_str()))
        .cloned()
        .collect();
    for key in moved {
        if let Some(v) = map.remove(&key) {
            inner.insert(key, v);
        }
    }
    map.insert(
        "anyOf".into(),
        Value::Array(vec![Value::Object(inner), json!({"type": "null"})]),
    );
}

fn root_for<T: JsonSchema>(id: &str) -> Value {
    let mut value = generator().into_root_schema_for::<T>().to_value();
    value["$id"] = Value::String(id.into());
    normalise(&mut value);
    value
}

/// `session-v1.6.json`: `SessionIngestRequest` with every nested type in `$defs`.
pub fn session_v1_6() -> Value {
    root_for::<SessionIngestRequest>(session_v1_6::SCHEMA_ID)
}

/// `session-v1.7.json`: the 1.7 `SessionIngestRequest` (1.6 plus tracking quality, SCH-1).
pub fn session_v1_7() -> Value {
    root_for::<session_v1_7::SessionIngestRequest>(session_v1_7::SCHEMA_ID)
}

/// `config-bundle-v1.json`: the signed `ConfigBundle`.
pub fn config_bundle_v1() -> Value {
    let mut value = root_for::<ConfigBundle>(config::CONFIG_BUNDLE_SCHEMA_ID);
    value["description"] = Value::String(
        "Signed ASD config bundle (catalog, baselines, scoring profile). Signature: Ed25519 over \
         the RFC 8785 form of the bundle without `signature`, `ed25519:` + base64url."
            .into(),
    );
    value
}

/// `api-v1.json`: one document whose `$defs` hold every `/v1` DTO (API-3).
pub fn api_v1() -> Value {
    let mut gen = generator();
    macro_rules! add {
        ($($t:ty),* $(,)?) => { $( gen.subschema_for::<$t>(); )* };
    }
    add!(
        SessionIngestRequest,
        SessionIngestResponse,
        ConfigBundle,
        ScoringProfile,
        Problem,
        InvalidParam,
        api_v1::LoginRequest,
        api_v1::RefreshRequest,
        api_v1::TokenPair,
        api_v1::User,
        api_v1::Role,
        api_v1::RegisterDeviceRequest,
        api_v1::RegisterDeviceResponse,
        api_v1::EnrolCodeRequest,
        api_v1::EnrolCodeResponse,
        api_v1::Device,
        api_v1::DevicePage,
        api_v1::Org,
        api_v1::OrgPatch,
        api_v1::OrgUser,
        api_v1::OrgUserPage,
        api_v1::CreateUserRequest,
        api_v1::UpdateUserRequest,
        api_v1::Participant,
        api_v1::ParticipantPage,
        api_v1::SessionAttentionFlag,
        api_v1::SessionSummary,
        api_v1::SessionDetail,
        api_v1::SessionPage,
        api_v1::MetricRow,
        api_v1::ClipScore,
        api_v1::ClipList,
        api_v1::Metrics,
        api_v1::ReviewItem,
        api_v1::ReviewPage,
        api_v1::CreateReviewRequest,
        api_v1::UpdateReviewRequest,
        api_v1::ConfigDocument,
        api_v1::ConfigDocumentPage,
        api_v1::CreateConfigDocumentRequest,
        api_v1::ConfigBundleSummary,
        api_v1::AuditEntry,
        api_v1::AuditPage,
        api_v1::Health,
        api_v1::Ready,
        care::CareEventsRequest,
        care::CareEventsResponse,
        care::CareSummary,
        care::ErasureRequestResponse,
        clinical::ClinicalReviewState,
        clinical::Disposition,
        clinical::ReviewVersionStatus,
        clinical::SignatureRole,
        clinical::CosignPolicy,
        clinical::WorklistScope,
        clinical::PatientRefMode,
        clinical::Paper,
        clinical::WorklistAssigned,
        clinical::ClinicalNoteKind,
        clinical::LetterKind,
        clinical::LetterStatus,
        clinical::TimelineEntryKind,
        clinical::SnapshotMeasurements,
        clinical::SnapshotClip,
        clinical::ClinicalReviewSnapshot,
        clinical::LetterSnapshot,
        clinical::UserProfile,
        clinical::UpdateProfileRequest,
        clinical::WorklistItem,
        clinical::WorklistPage,
        clinical::ClinicalSignature,
        clinical::ClinicalReviewVersion,
        clinical::ClinicalReview,
        clinical::ClaimClinicalReviewRequest,
        clinical::UpdateClinicalReviewRequest,
        clinical::SignRequest,
        clinical::AmendClinicalReviewRequest,
        clinical::PatientRef,
        clinical::PutPatientRefRequest,
        clinical::PatientRefSearchRequest,
        clinical::PatientRefSearchResponse,
        clinical::ClinicalNote,
        clinical::CreateClinicalNoteRequest,
        clinical::ClinicalNotePage,
        clinical::ReferralLetter,
        clinical::CreateLetterRequest,
        clinical::UpdateLetterRequest,
        clinical::LetterPage,
        clinical::TimelineEntry,
        clinical::TimelinePage,
        export::ExportFormat,
        export::ExportMode,
        export::ExportPurpose,
        export::ExportStatus,
        export::ExportKind,
        export::ExportFilters,
        export::ExportRequest,
        export::Export,
        export::ExportPage,
        export::DownloadUrl,
        export::DisclosureDelivery,
        export::Disclosure,
        export::DisclosurePage,
    );
    let mut defs = gen.take_definitions(true);
    defs.insert(
        "ProblemSlug".into(),
        json!({
            "description": "Problem slugs defined by the contract: `Problem.type` is \
                            `https://schemas.curv.org/asd/problems/<slug>`. Routes may add \
                            their own (e.g. `email-taken`, `last-owner`).",
            "type": "string",
            "enum": crate::problem::SLUGS,
        }),
    );
    let mut value = json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": API_SCHEMA_ID,
        "title": "ASD API v1",
        "description": "Request and response bodies of the ASD backend `/v1` API. Errors are \
                        `Problem` (application/problem+json). Each type is under `$defs`.",
        "$defs": Value::Object(defs),
    });
    normalise(&mut value);
    value
}

/// One exported schema file.
#[derive(Debug, Clone, PartialEq)]
pub struct SchemaDocument {
    pub file_name: &'static str,
    pub value: Value,
}

/// Every exported document, in a fixed order.
pub fn documents() -> Vec<SchemaDocument> {
    vec![
        SchemaDocument {
            file_name: SESSION_FILE,
            value: session_v1_6(),
        },
        SchemaDocument {
            file_name: SESSION_V1_7_FILE,
            value: session_v1_7(),
        },
        SchemaDocument {
            file_name: CONFIG_BUNDLE_FILE,
            value: config_bundle_v1(),
        },
        SchemaDocument {
            file_name: API_FILE,
            value: api_v1(),
        },
    ]
}

/// The committed text form: two-space pretty JSON, sorted keys, trailing newline.
pub fn render(value: &Value) -> String {
    let mut text = serde_json::to_string_pretty(value).expect("a JSON value always serialises");
    text.push('\n');
    text
}

/// Writes every document into `dir` (created if missing); returns the written paths.
pub fn export_all(dir: &Path) -> std::io::Result<Vec<PathBuf>> {
    std::fs::create_dir_all(dir)?;
    documents()
        .into_iter()
        .map(|doc| {
            let path = dir.join(doc.file_name);
            std::fs::write(&path, render(&doc.value))?;
            Ok(path)
        })
        .collect()
}

/// How a committed file differs from the export.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DriftKind {
    Missing,
    Unparseable(String),
    Differs,
}

/// One drifted file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Drift {
    pub file: String,
    pub kind: DriftKind,
}

impl fmt::Display for Drift {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            DriftKind::Missing => write!(f, "{}: missing", self.file),
            DriftKind::Unparseable(e) => write!(f, "{}: not JSON ({e})", self.file),
            DriftKind::Differs => write!(f, "{}: differs from the Rust contract types", self.file),
        }
    }
}

/// Compares the committed files in `dir` with a fresh export after JCS canonicalisation.
pub fn check(dir: &Path) -> Result<(), Vec<Drift>> {
    let drift: Vec<Drift> = documents()
        .into_iter()
        .filter_map(|doc| {
            let file = doc.file_name.to_string();
            let kind = match std::fs::read_to_string(dir.join(doc.file_name)) {
                Err(_) => DriftKind::Missing,
                Ok(text) => match serde_json::from_str::<Value>(&text) {
                    Err(e) => DriftKind::Unparseable(e.to_string()),
                    Ok(committed)
                        if canonical_bytes(&committed).ok() == canonical_bytes(&doc.value).ok() =>
                    {
                        return None
                    }
                    Ok(_) => DriftKind::Differs,
                },
            };
            Some(Drift { file, kind })
        })
        .collect();
    if drift.is_empty() {
        Ok(())
    } else {
        Err(drift)
    }
}

/// Sets `additionalProperties: false` on every object schema that declares `properties`
/// and says nothing about additional ones. For strict checks in tests and tooling only:
/// the contract itself tolerates unknown keys (Swift does).
pub fn forbid_additional_properties(value: &mut Value) {
    match value {
        Value::Object(map) => {
            for (key, child) in map.iter_mut() {
                if !DATA_KEYWORDS.contains(&key.as_str()) {
                    forbid_additional_properties(child);
                }
            }
            if map.contains_key("properties") && !map.contains_key("additionalProperties") {
                map.insert("additionalProperties".into(), Value::Bool(false));
            }
        }
        Value::Array(items) => items.iter_mut().for_each(forbid_additional_properties),
        _ => {}
    }
}

/// Validates `instance` against `schema` (JSON Schema 2020-12). Errors are
/// `"<instance pointer>: <message>"`, root as `/`.
#[cfg(feature = "validate")]
pub fn validate(schema: &Value, instance: &Value) -> Result<(), Vec<String>> {
    let compiled = jsonschema::JSONSchema::options()
        .with_draft(jsonschema::Draft::Draft202012)
        .compile(schema)
        .map_err(|e| vec![format!("invalid schema: {e}")])?;
    let result = compiled.validate(instance);
    match result {
        Ok(()) => Ok(()),
        Err(errors) => Err(errors
            .map(|e| {
                let path = e.instance_path.to_string();
                let path = if path.is_empty() {
                    "/".to_string()
                } else {
                    path
                };
                format!("{path}: {e}")
            })
            .collect()),
    }
}

/// Validates a session ingest payload against the exported session schema 1.6 (CON-4).
#[cfg(feature = "validate")]
pub fn validate_session(instance: &Value) -> Result<(), Vec<String>> {
    validate(&session_v1_6(), instance)
}

/// Validates a session ingest payload against the exported session schema 1.7 (SCH-1).
#[cfg(feature = "validate")]
pub fn validate_session_v1_7(instance: &Value) -> Result<(), Vec<String>> {
    validate(&session_v1_7(), instance)
}
