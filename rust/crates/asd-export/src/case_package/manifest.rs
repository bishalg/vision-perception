//! owner: WP-I.12
//!
//! `manifest.json` of a case package (SHR-2): [`CasePackageManifest`] and its JSON Schema
//! ([`manifest_schema`]), which `asd-cli schema export` writes as
//! `docs/spec/schemas/asd/case-package-v1.json` and `asd-cli schema check` compares.
//!
//! The manifest lists every other entry of the archive with its size and SHA-256
//! (`files[]`), and the entries a build could not produce (`absent[]`: the PDF when the
//! crate is built without the `pdf` feature). Serialisation is deterministic (struct field
//! order, no maps), so identical input gives identical bytes (XC-8).

use std::fmt;

use schemars::generate::SchemaSettings;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::policy::{Mode, Purpose};

/// `manifest.json` `schema` value of this version.
pub const SCHEMA_TAG: &str = "case-package/1.0";
/// File name of the exported schema in `docs/spec/schemas/asd/`.
pub const SCHEMA_FILE: &str = "case-package-v1.json";
/// `$id` of the exported schema.
pub const SCHEMA_ID: &str = "https://schemas.curv.org/asd/case-package-v1.json";
/// `recipient.label` of every de-identified package (SHR-2).
pub const DEIDENTIFIED_RECIPIENT: &str = "de-identified recipient";

/// The case package manifest (SHR-2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(
    title = "CasePackageManifest",
    description = "manifest.json of an ASD case package (share a case, SHR-1..6). Lists \
                   every other file of the ZIP with its size and SHA-256."
)]
pub struct CasePackageManifest {
    /// Always `case-package/1.0`.
    #[schemars(extend("const" = "case-package/1.0"))]
    pub schema: String,
    /// The export id (`exports.id`).
    pub package_id: String,
    /// RFC 3339 UTC time the package was rendered.
    pub created_at: String,
    pub generator: Generator,
    pub org: OrgInfo,
    #[schemars(with = "ModeSchema")]
    pub mode: Mode,
    #[schemars(with = "PurposeSchema")]
    pub purpose: Purpose,
    pub recipient: Recipient,
    /// Identified packages only: the consent the sharing relies on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub consent: Option<ConsentInfo>,
    pub participant: ParticipantInfo,
    /// Number of sessions in the package.
    pub sessions_n: u64,
    pub date_range: DateRange,
    /// Every entry of the archive except `manifest.json`, in archive order.
    pub files: Vec<FileEntry>,
    /// Entries this build could not produce (the PDF without the `pdf` feature).
    #[serde(default)]
    pub absent: Vec<AbsentFile>,
    /// The XC-6 watermark line.
    pub watermark: String,
    /// Always `true`: this is not a diagnostic tool (ADR-0004).
    #[schemars(extend("const" = true))]
    pub not_diagnostic: bool,
}

/// The software that rendered the package.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Generator {
    /// `asd-export`.
    #[serde(rename = "crate")]
    pub crate_name: String,
    pub version: String,
}

/// The sending clinic.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct OrgInfo {
    pub name: String,
    /// IANA time zone of the org.
    pub timezone: String,
}

/// Who the package is for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Recipient {
    /// Identified: the free text the sender entered. De-identified:
    /// `de-identified recipient`.
    pub label: String,
}

/// The consent an identified package relies on (SHR-7).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ConsentInfo {
    /// An identifier of the consent, such as a form number.
    pub reference: String,
    /// RFC 3339 time the consent was recorded.
    pub recorded_at: String,
    /// Display name of the staff member who recorded it.
    pub recorded_by_display: String,
}

/// The participant: a pseudonym (de-identified) or the token and, when given, the MRN.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum ParticipantInfo {
    Deidentified {
        /// `P-` pseudonym, stable within this package only (XC-4).
        pseudonym: String,
    },
    Identified {
        /// The participant token.
        token: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        mrn: Option<MrnInfo>,
    },
}

/// A hospital MRN. `Debug` never prints the value.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct MrnInfo {
    pub authority: String,
    pub value: String,
}

impl fmt::Debug for MrnInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MrnInfo")
            .field("authority", &self.authority)
            .field("value", &"<redacted>")
            .finish()
    }
}

/// First and last session: `YYYY-MM-DD` in the org time zone (identified) or `YYYY-MM`
/// UTC (de-identified, XC-4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DateRange {
    pub from: String,
    pub to: String,
}

/// One entry of the archive.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct FileEntry {
    /// Path inside the ZIP.
    pub path: String,
    pub media_type: String,
    /// Uncompressed size.
    pub bytes: u64,
    /// Lower-case hex SHA-256 of the uncompressed bytes.
    #[schemars(extend("pattern" = "^[0-9a-f]{64}$"))]
    pub sha256: String,
}

/// An entry this build did not produce.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AbsentFile {
    pub path: String,
    pub media_type: String,
    pub reason: String,
}

/// Schema of [`Mode`] (the policy enum carries no `JsonSchema`; same wire names).
#[derive(JsonSchema)]
#[serde(rename_all = "snake_case")]
#[allow(dead_code)]
enum ModeSchema {
    Identified,
    Deidentified,
}

/// Schema of [`Purpose`] (same wire names).
#[derive(JsonSchema)]
#[serde(rename_all = "snake_case")]
#[allow(dead_code)]
enum PurposeSchema {
    ClinicalCare,
    Research,
    QualityImprovement,
    PatientRequest,
}

/// JSON Schema (2020-12) of [`CasePackageManifest`], with `$id` [`SCHEMA_ID`].
pub fn manifest_schema() -> Value {
    let mut value = SchemaSettings::draft2020_12()
        .into_generator()
        .into_root_schema_for::<CasePackageManifest>()
        .to_value();
    value["$id"] = Value::String(SCHEMA_ID.to_owned());
    value
}

impl CasePackageManifest {
    /// Pretty JSON with a trailing newline, as written to the archive.
    pub fn to_json_bytes(&self) -> Vec<u8> {
        let mut out = serde_json::to_vec_pretty(self).expect("plain data serialises");
        out.push(b'\n');
        out
    }
}
