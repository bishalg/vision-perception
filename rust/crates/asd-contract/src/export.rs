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

// ---- integrations and deliveries (WP-I.7: INTG-1..5, DLV-1..6) ----------------------------

/// DLV-2: webhook-receiver download links live 15 minutes (EXJ-5 rules otherwise).
pub const WEBHOOK_DOWNLOAD_TTL_SECS: u64 = 900;
/// DLV-1: delays before the 2nd..6th delivery attempt (1 min, 5 min, 30 min, 2 h, 12 h);
/// the 6th failure marks the delivery `failed`.
pub const DELIVERY_BACKOFF_SECS: [u64; 5] = [60, 300, 1_800, 7_200, 43_200];
/// INTG-1: destination name, in characters.
pub const MAX_INTEGRATION_NAME_CHARS: usize = 120;
/// DLV-2: the prefix of a Standard Webhooks signing secret.
pub const WEBHOOK_SECRET_PREFIX: &str = "whsec_";

str_enum!(
    /// INTG-1 destination kind. `fhir_server` (WP-I.9) and `hl7_mllp` (WP-I.11) are refused
    /// until their channels exist.
    IntegrationKind {
        Webhook => "webhook",
        Sftp => "sftp",
        FhirServer => "fhir_server",
        Hl7Mllp => "hl7_mllp",
    }
);

str_enum!(
    /// INTG-1: only `production` destinations receive auto-deliveries (DLV-6).
    IntegrationEnvironment {
        Test => "test",
        Production => "production",
    }
);

str_enum!(
    /// INTG-1: a `disabled` destination is never delivered to.
    IntegrationStatus {
        Active => "active",
        Disabled => "disabled",
    }
);

str_enum!(
    /// INTG-3 `trigger`: `on_final_sign` delivers every finalised clinical review (DLV-6).
    DeliveryTrigger {
        Manual => "manual",
        OnFinalSign => "on_final_sign",
    }
);

str_enum!(
    /// DLV-5 delivery status.
    DeliveryStatus {
        Pending => "pending",
        Sending => "sending",
        Delivered => "delivered",
        Failed => "failed",
    }
);

/// DLV-2 webhook event type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub enum DeliveryEvent {
    /// A requested export is ready to download.
    #[serde(rename = "export.ready")]
    ExportReady,
    /// A clinical review was finalised; its export is ready (DLV-6).
    #[serde(rename = "review.finalized")]
    ReviewFinalized,
}

impl DeliveryEvent {
    pub const ALL: &'static [DeliveryEvent] = &[Self::ExportReady, Self::ReviewFinalized];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ExportReady => "export.ready",
            Self::ReviewFinalized => "review.finalized",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|e| e.as_str() == s)
    }
}

/// INTG-3 destination settings. One shape for every kind; the backend checks which keys a
/// kind needs. Unknown keys are refused, so nothing secret can be smuggled into `config`
/// (secrets go through `PUT /v1/integrations/{id}/secret`, INTG-2).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct IntegrationConfig {
    /// webhook: `https://` receiver URL, no user info (DLV-3).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// webhook: events the receiver wants (default: all). Auto-deliveries (DLV-6) go only
    /// to webhooks that accept `review.finalized`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub events: Vec<DeliveryEvent>,
    /// sftp: host name or address.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    /// sftp: port (default 22).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    /// sftp: absolute directory the files are dropped in.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_dir: Option<String>,
    /// sftp (INTG-5, mandatory): the server's host key fingerprint as printed by
    /// `ssh-keygen -lf` (`SHA256:<base64>`). Pinned: a mismatch fails the delivery.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host_key_sha256: Option<String>,
    /// sftp: prefix of dropped file names (`[A-Za-z0-9._-]`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_prefix: Option<String>,
    /// Format of auto-deliveries (required with `trigger = on_final_sign`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<ExportFormat>,
    /// Mode of auto-deliveries; DLV-6 auto-exports are `identified`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<ExportMode>,
    /// Default `manual`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trigger: Option<DeliveryTrigger>,
    /// HL7 v2 profile (INTG-3 `hl7 {...}`, HL7-2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hl7: Option<serde_json::Value>,
    /// FHIR profile (INTG-3 `fhir {...}`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fhir: Option<serde_json::Value>,
}

/// `POST /v1/orgs/{id}/integrations` (`IntegrationManage`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateIntegrationRequest {
    pub kind: IntegrationKind,
    /// 1..120 characters.
    pub name: String,
    /// Default `test`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub environment: Option<IntegrationEnvironment>,
    #[serde(default)]
    pub config: IntegrationConfig,
}

/// `PATCH /v1/integrations/{id}` (`IntegrationManage`). `config` replaces the whole config.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct UpdateIntegrationRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub environment: Option<IntegrationEnvironment>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<IntegrationStatus>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config: Option<IntegrationConfig>,
}

/// `PUT /v1/integrations/{id}/secret` (INTG-2, webhook only): `whsec_` + base64 of 24..64
/// random bytes. Write-only: never returned, logged or audited.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PutIntegrationSecretRequest {
    pub secret: String,
}

impl std::fmt::Debug for PutIntegrationSecretRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PutIntegrationSecretRequest { secret: [redacted] }")
    }
}

/// A delivery destination (INTG-1). Secrets never appear: `has_secret` and the first 8 hex
/// of the secret's SHA-256 (INTG-2); for SFTP, the public key it authenticates with.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Integration {
    pub id: String,
    pub org_id: String,
    pub kind: IntegrationKind,
    pub name: String,
    pub environment: IntegrationEnvironment,
    pub status: IntegrationStatus,
    pub config: IntegrationConfig,
    pub has_secret: bool,
    pub secret_fingerprint: Option<String>,
    /// sftp: OpenSSH public key to install at the hospital (INTG-5).
    pub ssh_public_key: Option<String>,
    #[schemars(extend("format" = "date-time"))]
    pub created_at: String,
    #[schemars(extend("format" = "date-time"))]
    pub updated_at: String,
    #[schemars(extend("format" = "date-time"))]
    pub last_test_at: Option<String>,
    pub last_test_ok: Option<bool>,
    /// Short error class of the last failed test (INTG-4).
    pub last_test_error: Option<String>,
}

/// `GET /v1/orgs/{id}/integrations` (`IntegrationRead`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct IntegrationPage {
    pub items: Vec<Integration>,
    pub next_cursor: Option<String>,
    pub total: Option<u64>,
}

/// `POST /v1/integrations/{id}/ssh-key` (INTG-5): generates (or rotates) the destination's
/// Ed25519 key and returns its public half.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SshPublicKey {
    /// `ssh-ed25519 AAAA... curv-asd-<destination>`.
    pub public_key: String,
    /// `SHA256:<base64>`, as `ssh-keygen -lf` prints it.
    pub fingerprint: String,
}

/// `POST /v1/integrations/{id}/test` (INTG-4): a synthetic delivery (token `TEST-0000`, no
/// participant data). Never carries the remote response body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct IntegrationTestResult {
    pub ok: bool,
    /// Short error class when not ok (`connect-failed`, `host-key-mismatch`, ...).
    pub error_class: Option<String>,
    /// E.g. `http 204` or the dropped file path.
    pub remote_ref: Option<String>,
    #[schemars(extend("format" = "date-time"))]
    pub tested_at: String,
}

/// One delivery of an export to a destination (DLV-1, DLV-5).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Delivery {
    pub id: String,
    pub export_id: String,
    pub destination_id: String,
    pub destination_name: String,
    pub event: DeliveryEvent,
    pub status: DeliveryStatus,
    pub attempts: u32,
    /// Enum spelling only, never remote text.
    pub last_error_class: Option<String>,
    pub remote_ref: Option<String>,
    #[schemars(extend("format" = "date-time"))]
    pub created_at: String,
    #[schemars(extend("format" = "date-time"))]
    pub updated_at: String,
    #[schemars(extend("format" = "date-time"))]
    pub delivered_at: Option<String>,
}

/// `GET /v1/integrations/{id}/deliveries` and `GET /v1/exports/{id}/deliveries`, newest
/// first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DeliveryPage {
    pub items: Vec<Delivery>,
    pub next_cursor: Option<String>,
    pub total: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integration_config_refuses_unknown_keys_and_redacts_secrets() {
        let c: IntegrationConfig = serde_json::from_value(serde_json::json!({
            "url": "https://his.example.org/hook", "events": ["review.finalized"],
            "trigger": "on_final_sign", "format": "hl7v2"
        }))
        .unwrap();
        assert_eq!(c.events, [DeliveryEvent::ReviewFinalized]);
        assert_eq!(c.trigger, Some(DeliveryTrigger::OnFinalSign));
        assert!(serde_json::from_value::<IntegrationConfig>(
            serde_json::json!({"password": "hunter2"})
        )
        .is_err());
        let s = PutIntegrationSecretRequest {
            secret: "whsec_abc".into(),
        };
        assert!(!format!("{s:?}").contains("abc"));
        for e in DeliveryEvent::ALL {
            assert_eq!(serde_json::to_value(e).unwrap(), e.as_str());
            assert_eq!(DeliveryEvent::parse(e.as_str()), Some(*e));
        }
        assert_eq!(DELIVERY_BACKOFF_SECS.len(), 5);
    }

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
