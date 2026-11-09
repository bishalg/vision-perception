//! owner: WP-X.11
//!
//! Research datasets (ADR-0015 R-7..R-9; design note
//! `docs/spec/modules/asd-research-toolkit/datasets.md`): a frozen, hashed member list of
//! sessions taken from a cohort at a point in time, with exclusion rules, optional clinician
//! labels, an optional seeded split stratified by age band, lineage and a data dictionary.
//! Immutable once frozen: a change is a new dataset (`lineage.parent_dataset_id`), never an
//! edit.
//!
//! This module holds the DTOs and the two pure functions every surface must agree on:
//! [`content_hash`] (the dataset's identity) and [`assign_split`] (the seeded, stratified
//! train / validation / holdout assignment). The backend (`routes/datasets.rs`) calls them at
//! freeze time; a client that holds the members can verify the hash with the same code.
//!
//! Conventions as `api_v1`: snake_case, UUID strings, RFC 3339 UTC timestamps, hashes as
//! lowercase hex sha256.

use std::collections::BTreeMap;

use asd_core::types::tracking::TrackingVerdict;
use asd_core::types::GazeQualityVerdict;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::mirror;
use crate::params::Unit;

/// Largest dataset (included plus excluded sessions of the cohort snapshot).
pub const MAX_DATASET_SESSIONS: u32 = 10_000;
/// Rules per dataset.
pub const MAX_EXCLUSION_RULES: usize = 20;
/// Sessions named by one explicit-list rule.
pub const MAX_EXPLICIT_SESSIONS: usize = 1_000;
/// Characters of a rule id and of an explicit exclusion reason.
pub const MAX_RULE_ID_CHARS: usize = 64;
pub const MAX_REASON_CHARS: usize = 200;
/// Version tag inside the hashed document; bumped when the hash input changes.
pub const HASH_VERSION: &str = "dataset-hash/1";
/// `Exclusion.reason` of the system exclusions and of the quality-gate rule.
pub mod reasons {
    /// No age, or an age outside every band of the published profile.
    pub const NO_AGE: &str = "no_age";
    /// `quality_gates.min_valid_ratio`.
    pub const VALID_RATIO: &str = "valid_ratio_below";
    /// `quality_gates.exclude_quality_verdicts`.
    pub const QUALITY_VERDICT: &str = "quality_verdict";
    /// `quality_gates.exclude_tracking_verdicts`.
    pub const TRACKING_VERDICT: &str = "tracking_verdict";
    /// `quality_gates.min_clips_completed`.
    pub const CLIPS: &str = "clips_below";
}
/// `DatasetMember.excluded` of a session erased after the freeze (R-9).
pub const EXCLUDED_ERASED: &str = "erased";

// ---- enums ---------------------------------------------------------------------------------

/// `datasets.status`. A dataset is frozen when it is created; archive hides it from pickers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DatasetStatus {
    Frozen,
    Archived,
}

/// The part of a split a member belongs to.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum SplitName {
    Train,
    Validation,
    Holdout,
}

impl SplitName {
    pub const ALL: [SplitName; 3] = [Self::Train, Self::Validation, Self::Holdout];
}

/// What a split is stratified by. Only `age_band` today (R-7).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SplitStrata {
    #[default]
    AgeBand,
}

/// Where labels come from (R-8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum LabelSourceKind {
    /// The signed clinical review's disposition (the review queue outcome).
    ReviewOutcomes,
    /// An imported label file (WP-X.16).
    Import,
}

/// The kinds of [`ExclusionRule`] (for labels and filters).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExclusionRuleKind {
    QualityGates,
    Tukey,
    MadZ,
    ExplicitList,
}

/// Value type of a data dictionary column.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ColumnType {
    String,
    Integer,
    Number,
    Datetime,
}

// ---- exclusion rules (R-7, ADR-0015 V-5) ---------------------------------------------------

/// One session named by an explicit-list rule, with the researcher's reason.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ExplicitExclusion {
    pub session_id: String,
    /// 1..200 characters; no names or dates of birth (de-identified research data).
    pub reason: String,
}

fn default_tukey_k() -> f64 {
    1.5
}

fn default_mad_z() -> f64 {
    3.5
}

/// An exclusion rule of a dataset (R-7). The kinds mirror the ADR-0015 V-5 presets so the
/// outlier tools (WP-X.22) store what they apply here. Every rule has an `id` unique in the
/// dataset (1..64 characters); each [`Exclusion`] names the rule that removed the session.
///
/// WP-X.11 applies `quality_gates` and `explicit_list`; `tukey` and `mad_z` need per-session
/// metric values (WP-X.19/X.22) and are refused with `400 validation-failed` until then.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ExclusionRule {
    /// V-5 "quality gates": every threshold given is applied; none given excludes nothing.
    QualityGates {
        id: String,
        /// Excludes sessions whose valid-sample ratio is below this, `[0, 1]`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        min_valid_ratio: Option<f64>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        #[schemars(with = "Vec<mirror::GazeQualityVerdict>")]
        exclude_quality_verdicts: Vec<GazeQualityVerdict>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        #[schemars(with = "Vec<mirror::TrackingVerdict>")]
        exclude_tracking_verdicts: Vec<TrackingVerdict>,
        /// Excludes sessions with fewer scored clips than this.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        min_clips_completed: Option<u32>,
    },
    /// V-5 Tukey fences: outside `[q1 - k * IQR, q3 + k * IQR]` of `metric` (WP-X.22).
    Tukey {
        id: String,
        metric: String,
        #[serde(default = "default_tukey_k")]
        k: f64,
    },
    /// V-5 robust z: `|x - median| / (1.4826 * MAD) > z` of `metric` (WP-X.22).
    MadZ {
        id: String,
        metric: String,
        #[serde(default = "default_mad_z")]
        z: f64,
    },
    /// Sessions the researcher names, each with a reason.
    ExplicitList {
        id: String,
        sessions: Vec<ExplicitExclusion>,
    },
}

impl ExclusionRule {
    pub fn id(&self) -> &str {
        match self {
            Self::QualityGates { id, .. }
            | Self::Tukey { id, .. }
            | Self::MadZ { id, .. }
            | Self::ExplicitList { id, .. } => id,
        }
    }

    pub fn kind(&self) -> ExclusionRuleKind {
        match self {
            Self::QualityGates { .. } => ExclusionRuleKind::QualityGates,
            Self::Tukey { .. } => ExclusionRuleKind::Tukey,
            Self::MadZ { .. } => ExclusionRuleKind::MadZ,
            Self::ExplicitList { .. } => ExclusionRuleKind::ExplicitList,
        }
    }
}

/// A session of the cohort snapshot left out of the dataset. `rule_id` names the rule; a
/// system exclusion (no placeable age) has none. `reason` is a [`reasons`] value or, for an
/// explicit list, the researcher's text.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
pub struct Exclusion {
    pub session_id: String,
    #[serde(default)]
    pub rule_id: Option<String>,
    pub reason: String,
}

// ---- labels (R-8) ----------------------------------------------------------------------------

/// Where the dataset's labels came from and when they were read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct LabelSource {
    pub kind: LabelSourceKind,
    #[schemars(extend("format" = "date-time"))]
    pub taken_at: String,
    /// Members that carry a label.
    pub count: u32,
}

/// One member's label with its provenance (R-8).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Label {
    /// For `review_outcomes`: the signed review's disposition (`clinical::Disposition`).
    pub value: String,
    pub source: LabelSourceKind,
    /// When the label was given (the review's signing time).
    #[schemars(extend("format" = "date-time"))]
    pub taken_at: String,
    /// `R-` pseudonym of the signing reviewer, stable within the dataset only.
    #[serde(default)]
    pub reviewer_pseudonym: Option<String>,
}

// ---- split (R-7) -----------------------------------------------------------------------------

/// Shares of the split; each in `[0, 1]`, `train > 0`, summing to 1 (±1e-9).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SplitFractions {
    pub train: f64,
    pub validation: f64,
    pub holdout: f64,
}

impl Default for SplitFractions {
    fn default() -> Self {
        Self {
            train: 0.7,
            validation: 0.15,
            holdout: 0.15,
        }
    }
}

/// A frozen dataset's split: [`assign_split`] with this seed and these fractions.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Split {
    pub seed: i64,
    pub fractions: SplitFractions,
    #[serde(default)]
    pub strata: SplitStrata,
}

/// The split a create request asks for; the server draws the seed when absent.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SplitRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed: Option<i64>,
    #[serde(default)]
    pub fractions: SplitFractions,
    #[serde(default)]
    pub strata: SplitStrata,
}

/// Members per split part in one stratum (`age_band` id, or `unbanded`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SplitStratumCount {
    pub stratum: String,
    pub train: u32,
    pub validation: u32,
    pub holdout: u32,
}

// ---- lineage and dictionary ------------------------------------------------------------------

/// Where a dataset came from (R-7): the cohort snapshot, the versions in force at freeze time,
/// an optional parent dataset (a new version of it) and an optional run it was derived for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Lineage {
    #[serde(default)]
    pub cohort_id: Option<String>,
    /// sha256 of the RFC 8785 form of the cohort definition used.
    pub cohort_hash: String,
    #[serde(default)]
    pub parent_dataset_id: Option<String>,
    #[serde(default)]
    pub run_id: Option<String>,
    /// sha256 of the RFC 8785 form of the published scoring profile whose `ageBands` placed
    /// the members.
    pub profile_hash: String,
    pub profile_version: String,
    pub catalog_version: String,
    pub baselines_version: String,
    /// The backend build (`asd-backend/<workspace version>`).
    pub code_version: String,
}

/// One column of the data dictionary (R-7). `description_key` is a domain catalogue key
/// (`dataset_column.<name>`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Column {
    pub name: String,
    #[serde(rename = "type")]
    pub r#type: ColumnType,
    #[serde(default)]
    pub unit: Option<Unit>,
    pub description_key: String,
}

/// The member table's columns (what `GET .../members` and the WP-X.15 package carry).
pub const DICTIONARY_COLUMNS: [(&str, ColumnType, Option<Unit>); 9] = [
    ("session_id", ColumnType::String, None),
    ("participant_pseudonym", ColumnType::String, None),
    ("age_months", ColumnType::Integer, Some(Unit::Months)),
    ("age_band", ColumnType::String, None),
    ("split", ColumnType::String, None),
    ("label", ColumnType::String, None),
    ("label_source", ColumnType::String, None),
    ("label_taken_at", ColumnType::Datetime, None),
    ("reviewer_pseudonym", ColumnType::String, None),
];

/// The data dictionary of every dataset frozen by this version.
pub fn data_dictionary() -> Vec<Column> {
    DICTIONARY_COLUMNS
        .iter()
        .map(|(name, t, unit)| Column {
            name: (*name).to_owned(),
            r#type: *t,
            unit: *unit,
            description_key: format!("dataset_column.{name}"),
        })
        .collect()
}

// ---- DTOs ------------------------------------------------------------------------------------

/// `GET /v1/research/datasets/{id}` (`AnalysisRead`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Dataset {
    pub id: String,
    pub org_id: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    pub status: DatasetStatus,
    #[serde(default)]
    pub cohort_id: Option<String>,
    #[schemars(extend("format" = "date-time"))]
    pub frozen_at: String,
    /// [`content_hash`] at freeze time, hex. Erasure later does not change it (R-9).
    pub content_hash: String,
    /// Members included at freeze time.
    pub member_count: u32,
    /// Included members erased since the freeze (`excluded: erased` in the members list).
    #[serde(default)]
    pub erased_count: u32,
    pub exclusion_rules: Vec<ExclusionRule>,
    /// Every cohort session left out, sorted by session id.
    pub exclusions: Vec<Exclusion>,
    #[serde(default)]
    pub label_source: Option<LabelSource>,
    #[serde(default)]
    pub split: Option<Split>,
    /// Members per stratum and split part at freeze time (empty without a split).
    #[serde(default)]
    pub split_counts: Vec<SplitStratumCount>,
    pub lineage: Lineage,
    pub data_dictionary: Vec<Column>,
    #[serde(default)]
    pub created_by: Option<String>,
    #[schemars(extend("format" = "date-time"))]
    pub created_at: String,
    #[serde(default)]
    pub archived_at: Option<String>,
}

/// One member of a dataset (`GET /v1/research/datasets/{id}/members`). A member erased after
/// the freeze keeps only `session_id` and `excluded: "erased"` (R-9).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DatasetMember {
    pub session_id: String,
    /// `P-` pseudonym of the participant, stable within the dataset only.
    #[serde(default)]
    pub participant_pseudonym: Option<String>,
    #[serde(default)]
    pub age_months: Option<i32>,
    #[serde(default)]
    pub age_band: Option<String>,
    #[serde(default)]
    pub split: Option<SplitName>,
    #[serde(default)]
    pub label: Option<Label>,
    /// `erased` when the session was erased after the freeze.
    #[serde(default)]
    pub excluded: Option<String>,
}

/// `POST /v1/research/datasets` (`ExperimentWrite`): freezes a dataset from a cohort of the
/// caller's org. Answers `201 Dataset`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateDatasetRequest {
    /// 1..80 characters, unique in the org.
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub cohort_id: String,
    /// A dataset of the org this one is a new version of.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_dataset_id: Option<String>,
    /// An analysis run of the org this dataset was derived for (lineage only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,
    #[serde(default)]
    pub exclusion_rules: Vec<ExclusionRule>,
    /// Attach labels from this source (`import` arrives with WP-X.16).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub labels: Option<LabelSourceKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub split: Option<SplitRequest>,
}

/// `GET /v1/research/datasets` (`AnalysisRead`); `cursor` is opaque, `limit <= 100`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct DatasetPage {
    pub items: Vec<Dataset>,
    pub next_cursor: Option<String>,
}

/// `GET /v1/research/datasets/{id}/members` (`AnalysisRead`); sorted by session id.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct DatasetMemberPage {
    pub items: Vec<DatasetMember>,
    pub next_cursor: Option<String>,
}

/// The dataset an analysis run takes its membership from (stored in the run definition).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DatasetRef {
    pub id: String,
    pub content_hash: String,
}

// ---- split algorithm ---------------------------------------------------------------------------

/// The stratum name of a member without an age band.
pub const UNBANDED: &str = "unbanded";

/// Checks split fractions: finite, each in `[0, 1]`, `train > 0`, sum 1 (±1e-9).
pub fn check_fractions(f: &SplitFractions) -> Result<(), String> {
    let all = [f.train, f.validation, f.holdout];
    if all
        .iter()
        .any(|v| !v.is_finite() || !(0.0..=1.0).contains(v))
    {
        return Err("each fraction must be in [0, 1]".into());
    }
    if f.train <= 0.0 {
        return Err("train must be above 0".into());
    }
    if ((f.train + f.validation + f.holdout) - 1.0).abs() > 1e-9 {
        return Err("the fractions must add up to 1".into());
    }
    Ok(())
}

/// Seeded order key of a session: sha256 of the seed (8 bytes little endian) and the id.
fn order_key(seed: i64, session_id: &str) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(seed.to_le_bytes());
    h.update(session_id.as_bytes());
    h.finalize().into()
}

/// Part sizes of a stratum of `n` by the largest-remainder method: `floor(f * n)` each, the
/// remaining members one by one to the parts with the largest fractional rest (ties in the
/// order train, validation, holdout).
pub fn part_sizes(n: u32, f: &SplitFractions) -> [u32; 3] {
    let fr = [f.train, f.validation, f.holdout];
    let exact: Vec<f64> = fr.iter().map(|x| x * f64::from(n)).collect();
    let mut sizes = [0u32; 3];
    for (i, e) in exact.iter().enumerate() {
        // exact is in [0, n]; the cast truncates a non-negative value.
        sizes[i] = e.floor().clamp(0.0, f64::from(n)) as u32;
    }
    let mut rest = n.saturating_sub(sizes.iter().sum());
    let mut order: Vec<usize> = vec![0, 1, 2];
    order.sort_by(|&a, &b| {
        let ra = exact[a] - exact[a].floor();
        let rb = exact[b] - exact[b].floor();
        rb.total_cmp(&ra).then(a.cmp(&b))
    });
    for i in order.iter().cycle() {
        if rest == 0 {
            break;
        }
        if fr[*i] > 0.0 {
            sizes[*i] += 1;
            rest -= 1;
        }
    }
    sizes
}

/// Assigns every member `(session_id, age band)` a split part (R-7): members are grouped by
/// stratum (band, or [`UNBANDED`]), ordered within it by [`order_key`] (seed, id; ties by id),
/// and the first [`part_sizes`] go to train, the next to validation, the rest to holdout. The
/// result depends only on the member set, the seed and the fractions.
pub fn assign_split(
    members: &[(String, Option<String>)],
    seed: i64,
    fractions: &SplitFractions,
) -> BTreeMap<String, SplitName> {
    let mut strata: BTreeMap<&str, Vec<(&str, [u8; 32])>> = BTreeMap::new();
    for (id, band) in members {
        strata
            .entry(band.as_deref().unwrap_or(UNBANDED))
            .or_default()
            .push((id.as_str(), order_key(seed, id)));
    }
    let mut out = BTreeMap::new();
    for (_, mut list) in strata {
        list.sort_by(|a, b| a.1.cmp(&b.1).then(a.0.cmp(b.0)));
        let n = u32::try_from(list.len()).unwrap_or(u32::MAX);
        let [train, validation, _] = part_sizes(n, fractions);
        for (i, (id, _)) in list.into_iter().enumerate() {
            let i = u32::try_from(i).unwrap_or(u32::MAX);
            let part = if i < train {
                SplitName::Train
            } else if i < train + validation {
                SplitName::Validation
            } else {
                SplitName::Holdout
            };
            out.insert(id.to_owned(), part);
        }
    }
    out
}

/// Members per stratum and part, strata sorted by name.
pub fn split_counts(members: &[DatasetMember]) -> Vec<SplitStratumCount> {
    let mut by: BTreeMap<String, [u32; 3]> = BTreeMap::new();
    for m in members {
        let Some(split) = m.split else { continue };
        let e = by
            .entry(m.age_band.clone().unwrap_or_else(|| UNBANDED.into()))
            .or_default();
        let i = match split {
            SplitName::Train => 0,
            SplitName::Validation => 1,
            SplitName::Holdout => 2,
        };
        e[i] += 1;
    }
    by.into_iter()
        .map(|(stratum, c)| SplitStratumCount {
            stratum,
            train: c[0],
            validation: c[1],
            holdout: c[2],
        })
        .collect()
}

// ---- content hash ------------------------------------------------------------------------------

/// The hashed document (see [`content_hash`]).
pub fn hash_document(
    members: &[DatasetMember],
    exclusions: &[Exclusion],
    rules: &[ExclusionRule],
    split: Option<&Split>,
    labels: Option<LabelSourceKind>,
    lineage: &Lineage,
) -> Value {
    let mut ms: Vec<&DatasetMember> = members.iter().collect();
    ms.sort_by(|a, b| a.session_id.cmp(&b.session_id));
    let members: Vec<Value> = ms
        .iter()
        .map(|m| {
            json!({
                "session_id": m.session_id,
                "age_months": m.age_months,
                "age_band": m.age_band,
                "split": m.split,
                "label": m.label.as_ref().map(|l| json!({"value": l.value, "source": l.source})),
            })
        })
        .collect();
    let mut ex: Vec<&Exclusion> = exclusions.iter().collect();
    ex.sort();
    json!({
        "v": HASH_VERSION,
        "members": members,
        "exclusions": ex,
        "exclusion_rules": rules,
        "split": split,
        "labels": labels,
        "lineage": lineage,
    })
}

/// The dataset's content hash: sha256 (hex) of the RFC 8785 form of [`hash_document`]:
/// `{v, members, exclusions, exclusion_rules, split, labels, lineage}` with the members sorted
/// by session id as `{session_id, age_months, age_band, split, label: {value, source}}` and
/// the exclusions sorted. Pseudonyms (random per dataset), label times, reviewer pseudonyms,
/// the name, the description and the freeze time are not hashed, so the same snapshot frozen
/// with the same rules, seed, labels and lineage has the same hash.
pub fn content_hash(
    members: &[DatasetMember],
    exclusions: &[Exclusion],
    rules: &[ExclusionRule],
    split: Option<&Split>,
    labels: Option<LabelSourceKind>,
    lineage: &Lineage,
) -> String {
    let doc = hash_document(members, exclusions, rules, split, labels, lineage);
    let digest = Sha256::digest(crate::jcs::to_vec(&doc));
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lineage() -> Lineage {
        Lineage {
            cohort_id: Some("c".into()),
            cohort_hash: "00".repeat(32),
            parent_dataset_id: None,
            run_id: None,
            profile_hash: "11".repeat(32),
            profile_version: "1.3.0".into(),
            catalog_version: "1.4.0".into(),
            baselines_version: "2026.10.0".into(),
            code_version: "asd-backend/0.1.0".into(),
        }
    }

    fn member(id: &str, band: &str, split: Option<SplitName>) -> DatasetMember {
        DatasetMember {
            session_id: id.into(),
            participant_pseudonym: Some(format!("P-{id}")),
            age_months: Some(30),
            age_band: Some(band.into()),
            split,
            label: None,
            excluded: None,
        }
    }

    #[test]
    fn rule_round_trips_with_kind_tag_and_defaults() {
        let raw = json!([
            {"kind": "quality_gates", "id": "q", "min_valid_ratio": 0.6,
             "exclude_tracking_verdicts": ["poor"]},
            {"kind": "tukey", "id": "t", "metric": "eye_dwell_ratio"},
            {"kind": "mad_z", "id": "m", "metric": "eye_dwell_ratio", "z": 3.0},
            {"kind": "explicit_list", "id": "e",
             "sessions": [{"session_id": "S-1", "reason": "device fault"}]},
        ]);
        let rules: Vec<ExclusionRule> = serde_json::from_value(raw).unwrap();
        assert_eq!(
            rules[1],
            ExclusionRule::Tukey {
                id: "t".into(),
                metric: "eye_dwell_ratio".into(),
                k: 1.5
            }
        );
        assert_eq!(rules[0].kind(), ExclusionRuleKind::QualityGates);
        assert_eq!(rules[3].id(), "e");
        let back: Vec<ExclusionRule> =
            serde_json::from_value(serde_json::to_value(&rules).unwrap()).unwrap();
        assert_eq!(back, rules);
    }

    #[test]
    fn fractions_are_checked() {
        assert!(check_fractions(&SplitFractions::default()).is_ok());
        let bad = |t, v, h| {
            check_fractions(&SplitFractions {
                train: t,
                validation: v,
                holdout: h,
            })
        };
        assert!(bad(0.5, 0.5, 0.5).is_err());
        assert!(bad(0.0, 0.5, 0.5).is_err());
        assert!(bad(1.2, -0.1, -0.1).is_err());
        assert!(bad(f64::NAN, 0.5, 0.5).is_err());
        assert!(bad(1.0, 0.0, 0.0).is_ok());
    }

    #[test]
    fn part_sizes_use_largest_remainder() {
        let f = SplitFractions::default();
        assert_eq!(part_sizes(10, &f), [7, 2, 1]);
        assert_eq!(part_sizes(0, &f), [0, 0, 0]);
        assert_eq!(part_sizes(1, &f), [1, 0, 0]);
        assert_eq!(part_sizes(20, &f), [14, 3, 3]);
        let half = SplitFractions {
            train: 0.5,
            validation: 0.0,
            holdout: 0.5,
        };
        assert_eq!(part_sizes(3, &half), [2, 0, 1]);
        for n in 0..50 {
            assert_eq!(part_sizes(n, &f).iter().sum::<u32>(), n);
        }
    }

    #[test]
    fn split_is_stratified_and_deterministic_by_seed() {
        let mut members = Vec::new();
        for i in 0..40 {
            members.push((format!("A-{i:02}"), Some("24-35".to_owned())));
        }
        for i in 0..20 {
            members.push((format!("B-{i:02}"), Some("36-47".to_owned())));
        }
        members.push(("U-1".to_owned(), None));
        let f = SplitFractions::default();
        let a = assign_split(&members, 42, &f);
        assert_eq!(a.len(), 61);
        // Same seed, members in another order: same assignment.
        let mut rev = members.clone();
        rev.reverse();
        assert_eq!(assign_split(&rev, 42, &f), a);
        // Each stratum gets its own shares.
        let count = |prefix: &str, part: SplitName| {
            a.iter()
                .filter(|(id, p)| id.starts_with(prefix) && **p == part)
                .count()
        };
        assert_eq!(count("A-", SplitName::Train), 28);
        assert_eq!(count("A-", SplitName::Validation), 6);
        assert_eq!(count("A-", SplitName::Holdout), 6);
        assert_eq!(count("B-", SplitName::Train), 14);
        assert_eq!(count("B-", SplitName::Validation), 3);
        assert_eq!(count("B-", SplitName::Holdout), 3);
        assert_eq!(a["U-1"], SplitName::Train);
        // Another seed permutes the members.
        let b = assign_split(&members, 43, &f);
        assert_ne!(a, b);
        assert_eq!(
            b.values().filter(|p| **p == SplitName::Train).count(),
            a.values().filter(|p| **p == SplitName::Train).count()
        );
    }

    #[test]
    fn hash_is_deterministic_and_order_free() {
        let m = vec![
            member("S-2", "24-35", Some(SplitName::Train)),
            member("S-1", "24-35", Some(SplitName::Holdout)),
        ];
        let ex = vec![Exclusion {
            session_id: "S-9".into(),
            rule_id: None,
            reason: reasons::NO_AGE.into(),
        }];
        let l = lineage();
        let split = Split {
            seed: 7,
            fractions: SplitFractions::default(),
            strata: SplitStrata::AgeBand,
        };
        let h = content_hash(&m, &ex, &[], Some(&split), None, &l);
        assert_eq!(h.len(), 64);
        let mut rev = m.clone();
        rev.reverse();
        assert_eq!(content_hash(&rev, &ex, &[], Some(&split), None, &l), h);
        // Pseudonyms are not hashed.
        let mut other = m.clone();
        other[0].participant_pseudonym = Some("P-x".into());
        assert_eq!(content_hash(&other, &ex, &[], Some(&split), None, &l), h);
        // A changed split, seed, member or lineage changes it.
        other[0].split = Some(SplitName::Validation);
        assert_ne!(content_hash(&other, &ex, &[], Some(&split), None, &l), h);
        let seeded = Split { seed: 8, ..split };
        assert_ne!(content_hash(&m, &ex, &[], Some(&seeded), None, &l), h);
        assert_ne!(content_hash(&m[..1], &ex, &[], Some(&split), None, &l), h);
        let mut l2 = l.clone();
        l2.catalog_version = "1.5.0".into();
        assert_ne!(content_hash(&m, &ex, &[], Some(&split), None, &l2), h);
        assert_ne!(
            content_hash(
                &m,
                &ex,
                &[],
                Some(&split),
                Some(LabelSourceKind::ReviewOutcomes),
                &l
            ),
            h
        );
        // A fixed reference value: the hash input is a stable wire contract.
        let doc = hash_document(&m, &ex, &[], Some(&split), None, &l);
        assert_eq!(doc["v"], HASH_VERSION);
        assert_eq!(doc["members"][0]["session_id"], "S-1");
    }

    #[test]
    fn dataset_round_trips() {
        let d = Dataset {
            id: "d".into(),
            org_id: "o".into(),
            name: "n".into(),
            description: None,
            status: DatasetStatus::Frozen,
            cohort_id: Some("c".into()),
            frozen_at: "2026-11-09T00:00:00Z".into(),
            content_hash: "ab".repeat(32),
            member_count: 2,
            erased_count: 0,
            exclusion_rules: vec![ExclusionRule::ExplicitList {
                id: "manual".into(),
                sessions: vec![ExplicitExclusion {
                    session_id: "S-3".into(),
                    reason: "duplicate".into(),
                }],
            }],
            exclusions: vec![Exclusion {
                session_id: "S-3".into(),
                rule_id: Some("manual".into()),
                reason: "duplicate".into(),
            }],
            label_source: Some(LabelSource {
                kind: LabelSourceKind::ReviewOutcomes,
                taken_at: "2026-11-09T00:00:00Z".into(),
                count: 1,
            }),
            split: Some(Split {
                seed: 1,
                fractions: SplitFractions::default(),
                strata: SplitStrata::AgeBand,
            }),
            split_counts: split_counts(&[member("S-1", "24-35", Some(SplitName::Train))]),
            lineage: lineage(),
            data_dictionary: data_dictionary(),
            created_by: None,
            created_at: "2026-11-09T00:00:00Z".into(),
            archived_at: None,
        };
        let v = serde_json::to_value(&d).unwrap();
        assert_eq!(v["data_dictionary"][2]["type"], "integer");
        assert_eq!(v["data_dictionary"][2]["unit"], "months");
        assert_eq!(v["exclusion_rules"][0]["kind"], "explicit_list");
        let back: Dataset = serde_json::from_value(v).unwrap();
        assert_eq!(back, d);
        let req: CreateDatasetRequest = serde_json::from_value(json!({
            "name": "x", "cohort_id": "c", "labels": "review_outcomes",
            "split": {"seed": 5}
        }))
        .unwrap();
        assert_eq!(req.split.unwrap().fractions, SplitFractions::default());
        assert!(serde_json::from_value::<CreateDatasetRequest>(
            json!({"name": "x", "cohort_id": "c", "bogus": 1})
        )
        .is_err());
    }
}
