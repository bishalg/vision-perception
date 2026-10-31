//! owner: WP-I.6
//!
//! Export job contracts (CTR-1, EXJ-1..9): the request body of `POST /v1/orgs/{id}/exports`,
//! the `Export` resource, the short-lived download URL and the per-participant accounting
//! of disclosures. Spec ids refer to
//! `docs/spec/modules/asd-clinician-and-interop/technical.md`.
//!
//! `ExportMode` and `ExportPurpose` carry the same snake_case names as
//! `asd_export::policy::{Mode, Purpose}`; the backend maps one onto the other (this crate
//! does not depend on `asd-export`, which depends on it).
//!
//! Conventions as `api_v1`: snake_case, UUID strings, RFC 3339 UTC timestamps.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

// ---- limits ------------------------------------------------------------------------------

/// EXJ-1: `reason`, in characters (shown in the audit log).
pub const MIN_REASON_CHARS: usize = 10;
pub const MAX_REASON_CHARS: usize = 500;
/// EXJ-9: sessions per export.
pub const MAX_EXPORT_ROWS: usize = 200_000;
/// EXJ-9: largest stored artifact (500 MB, `BlobStore::put_object_large`).
pub const MAX_ARTIFACT_BYTES: usize = 500 * 1024 * 1024;
/// EXJ-4: artifact lifetime when the org sets none, and the allowed range of
/// `settings.exports.ttl_hours`.
pub const DEFAULT_TTL_HOURS: u32 = 72;
pub const MIN_TTL_HOURS: u32 = 1;
pub const MAX_TTL_HOURS: u32 = 168;
/// EXJ-5: lifetime of a download URL.
pub const DOWNLOAD_URL_TTL_SECS: u64 = 300;
/// EXJ-1: at most this many ids in each id filter.
pub const MAX_FILTER_IDS: usize = 1_000;

/// `organizations.settings` keys owned by exports: `{"exports": {"ttl_hours": 72}}`.
pub mod settings {
    pub const EXPORTS: &str = "exports";
    pub const TTL_HOURS: &str = "ttl_hours";
}

// ---- enumerations ------------------------------------------------------------------------

macro_rules! str_enum {
    ($(#[$doc:meta])* $name:ident { $($variant:ident => $s:literal),* $(,)? }) => {
        $(#[$doc])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
        #[serde(rename_all = "snake_case")]
        pub enum $name { $($variant),* }

        impl $name {
            pub const ALL: &'static [$name] = &[$($name::$variant),*];

            /// The wire and database spelling.
            pub const fn as_str(self) -> &'static str {
                match self { $($name::$variant => $s),* }
            }

            pub fn parse(s: &str) -> Option<Self> {
                match s { $($s => Some($name::$variant),)* _ => None }
            }
        }
    };
}

str_enum!(
    /// EXJ-1 artifact format. `csv`, `csv_excel`, `csv_sheets`: ZIP of the four tables
    /// plus `manifest.json` in the three CSV dialects (CSV-1, CSV-3); `xlsx`: one workbook
    /// (XLSX-1); `pdf_summary`: PDF/A clinical summary (WP-I.5, not available yet);
    /// `fhir_bundle`: one FHIR R4 clinical report bundle, exactly one session
    /// (`filters.session_ids`); `fhir_ndjson`: one bundle per line, any number of sessions;
    /// `hl7v2`: one ORU^R01 message, exactly one session, identified only (HL7-7).
    ExportFormat {
        Csv => "csv",
        CsvExcel => "csv_excel",
        CsvSheets => "csv_sheets",
        Xlsx => "xlsx",
        PdfSummary => "pdf_summary",
        FhirBundle => "fhir_bundle",
        FhirNdjson => "fhir_ndjson",
        Hl7v2 => "hl7v2",
    }
);

str_enum!(
    /// EXJ-1: identified (token, MRN and clinical text where the requester may read them) or
    /// de-identified (XC-4).
    ExportMode {
        Identified => "identified",
        Deidentified => "deidentified",
    }
);

str_enum!(
    /// EXJ-1: why the export is made; decides which modes and sessions are allowed (XC-5).
    ExportPurpose {
        ClinicalCare => "clinical_care",
        Research => "research",
        QualityImprovement => "quality_improvement",
        PatientRequest => "patient_request",
    }
);

str_enum!(
    /// `exports.status`. `expired`: the artifact was deleted after `expires_at`; the
    /// accounting stays (EXJ-4).
    ExportStatus {
        Queued => "queued",
        Running => "running",
        Succeeded => "succeeded",
        Failed => "failed",
        Expired => "expired",
    }
);

str_enum!(
    /// `exports.kind`: a stored job artifact or a single synchronous document render (EXJ-7).
    ExportKind {
        Bulk => "bulk",
        Document => "document",
    }
);

// ---- request -----------------------------------------------------------------------------

/// EXJ-1 filters: the session-list filters plus id and clinical-state lists. Every field
/// is optional; an empty filter selects every session of the org.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ExportFilters {
    /// Attention flag (`green|yellow|red|unreliable`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flag: Option<String>,
    /// Quality verdict.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quality: Option<String>,
    /// `started_at` lower bound: RFC 3339 or `YYYY-MM-DD` (start of day, UTC).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    /// `started_at` upper bound: RFC 3339 or `YYYY-MM-DD` (end of day, UTC).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bundle_version: Option<String>,
    /// Whether the session has a finished review-queue item.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reviewed: Option<bool>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub participant_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub session_ids: Vec<String>,
    /// Clinical review states (`open`, `final`, ...); `none` selects sessions without a
    /// clinical review.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub clinical_state: Vec<String>,
}

/// `POST /v1/orgs/{id}/exports` (EXJ-1). Answers `202` with the [`Export`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ExportRequest {
    pub format: ExportFormat,
    pub mode: ExportMode,
    pub purpose: ExportPurpose,
    #[serde(default)]
    pub filters: ExportFilters,
    /// Delivery destinations (WP-I.7); empty = download only.
    #[serde(default)]
    pub destination_ids: Vec<String>,
    /// 10..500 characters, shown in the audit log.
    pub reason: String,
}

// ---- resources ---------------------------------------------------------------------------

/// An export (EXJ-1..4). Never carries clinical content: counts, hashes and the request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(example = example_export())]
pub struct Export {
    pub id: String,
    pub org_id: String,
    pub kind: ExportKind,
    pub format: ExportFormat,
    pub mode: ExportMode,
    pub purpose: ExportPurpose,
    pub status: ExportStatus,
    pub filters: ExportFilters,
    pub reason: String,
    pub destination_ids: Vec<String>,
    pub requested_by: Option<String>,
    #[schemars(extend("format" = "date-time"))]
    pub created_at: String,
    #[schemars(extend("format" = "date-time"))]
    pub completed_at: Option<String>,
    /// When the artifact is deleted (EXJ-4).
    #[schemars(extend("format" = "date-time"))]
    pub expires_at: Option<String>,
    /// Sessions in the artifact.
    pub row_count: Option<u64>,
    pub byte_size: Option<u64>,
    /// Hex SHA-256 of the artifact.
    pub sha256: Option<String>,
    pub file_name: Option<String>,
    pub content_type: Option<String>,
    /// Excluded sessions by reason (`withdrawn`, `no_research_consent`, `mode_not_allowed`,
    /// `capture_source_refused`).
    pub excluded: BTreeMap<String, u64>,
    /// Short error class when `failed`; never patient data.
    pub error: Option<String>,
}

fn example_export() -> Export {
    Export {
        id: "4f0e7a52-6a8c-4c3e-9a51-2f6b0c1d9e01".into(),
        org_id: "0b7e8f1c-2d3a-4b5c-8d9e-0f1a2b3c4d5e".into(),
        kind: ExportKind::Bulk,
        format: ExportFormat::Csv,
        mode: ExportMode::Deidentified,
        purpose: ExportPurpose::Research,
        status: ExportStatus::Succeeded,
        filters: ExportFilters {
            from: Some("2026-10-01".into()),
            ..ExportFilters::default()
        },
        reason: "Quarterly research extract for the ethics-approved cohort study".into(),
        destination_ids: Vec::new(),
        requested_by: Some("8d2c1b0a-9f8e-4d7c-b6a5-4e3d2c1b0a9f".into()),
        created_at: "2026-10-31T09:00:00.000Z".into(),
        completed_at: Some("2026-10-31T09:00:04.000Z".into()),
        expires_at: Some("2026-11-03T09:00:04.000Z".into()),
        row_count: Some(42),
        byte_size: Some(18_204),
        sha256: Some("9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08".into()),
        file_name: Some("export-4f0e7a52-csv.zip".into()),
        content_type: Some("application/zip".into()),
        excluded: BTreeMap::from([("no_research_consent".into(), 3), ("withdrawn".into(), 1)]),
        error: None,
    }
}

/// `GET /v1/orgs/{id}/exports`, newest first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ExportPage {
    pub items: Vec<Export>,
    pub next_cursor: Option<String>,
    pub total: Option<u64>,
}

/// `POST /v1/exports/{id}/download-url` (EXJ-5): a pre-signed GET (absolute) or a relative
/// `/v1/exports/{id}/file?t=...` (proxy blob mode), valid five minutes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DownloadUrl {
    pub url: String,
    #[schemars(extend("format" = "date-time"))]
    pub expires_at: String,
}

/// One delivery of an export (DLV-5), as listed with a disclosure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DisclosureDelivery {
    pub destination_id: String,
    pub destination_name: String,
    /// `pending | sending | delivered | failed`.
    pub status: String,
    #[schemars(extend("format" = "date-time"))]
    pub delivered_at: Option<String>,
}

/// One export that included the participant (EXJ-6, accounting of disclosures).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Disclosure {
    pub export_id: String,
    pub kind: ExportKind,
    pub format: ExportFormat,
    pub mode: ExportMode,
    pub purpose: ExportPurpose,
    pub status: ExportStatus,
    pub requested_by: Option<String>,
    #[schemars(extend("format" = "date-time"))]
    pub created_at: String,
    /// The participant's sessions in the export.
    pub session_ids: Vec<String>,
    pub deliveries: Vec<DisclosureDelivery>,
}

/// `GET /v1/participants/{id}/disclosures` (`ClinicalContentRead`), newest first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DisclosurePage {
    pub items: Vec<Disclosure>,
    pub next_cursor: Option<String>,
    pub total: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_defaults_and_unknown_fields() {
        let r: ExportRequest = serde_json::from_value(serde_json::json!({
            "format": "csv_excel", "mode": "deidentified", "purpose": "research",
            "reason": "Quarterly extract"
        }))
        .unwrap();
        assert_eq!(r.format, ExportFormat::CsvExcel);
        assert_eq!(r.filters, ExportFilters::default());
        assert!(r.destination_ids.is_empty());
        assert!(serde_json::from_value::<ExportRequest>(serde_json::json!({
            "format": "csv", "mode": "identified", "purpose": "research",
            "reason": "Quarterly extract", "mrn": "x"
        }))
        .is_err());
        assert!(serde_json::from_value::<ExportFilters>(serde_json::json!({"mrn": "x"})).is_err());
    }

    #[test]
    fn wire_names_round_trip() {
        for f in ExportFormat::ALL {
            assert_eq!(ExportFormat::parse(f.as_str()), Some(*f));
            assert_eq!(serde_json::to_value(f).unwrap(), f.as_str());
        }
        for s in ExportStatus::ALL {
            assert_eq!(serde_json::to_value(s).unwrap(), s.as_str());
        }
        assert_eq!(
            ExportPurpose::QualityImprovement.as_str(),
            "quality_improvement"
        );
    }
}
