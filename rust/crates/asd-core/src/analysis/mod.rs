//! owner: WP-X.1
//!
//! Researcher toolkit analysis core (ADR-0012; `docs/spec/modules/asd-research-toolkit/`):
//!
//! - [`profile`]: the 1.2.0 `ScoringProfile` sections the core reads, as core types (PRM-7..9);
//! - [`evaluate()`]: one session at `score` or `frames` level with a profile (RPX-2, RPX-3);
//! - [`aggregate`]: per-band distributions of a run, bootstrap CIs (RUN-4);
//! - [`suppress()`]: small-cell suppression of a run summary (PRV-1);
//! - [`sweep`]: parameter-sweep grids and per-cell aggregation (SWP-1..3, WP-X.9);
//! - [`age_profile`]: per-band shares against the baseline ranges (AGE-1, WP-X.9).
//!
//! Why the core has its own copies of contract shapes: `asd-contract` depends on `asd-core`,
//! so the core cannot name `asd_contract::config::ScoringProfile` or
//! `asd_contract::research::{RunSummary, Dist, ..}`. The types here carry the same members with
//! the same serde spelling, so the JSON of one decodes as the other (`tests/analysis.rs`
//! round-trips both directions); callers convert through `serde_json::Value` or, once the
//! contract re-exports them (as it does `TrackingQualityParams`), not at all.
//!
//! Everything here is pure and deterministic (RPX-6, RUN-5): no clock, no I/O, and the only
//! randomness is the seeded bootstrap of [`aggregate`].

pub mod age_profile;
pub mod aggregate;
pub mod evaluate;
pub mod profile;
pub mod suppress;
pub mod sweep;

use serde::{Deserialize, Serialize};

pub use age_profile::{
    AgeProfileAggregator, AgeProfileBand, AgeProfileBaselines, AgeProfileExpectation,
    AgeProfileSide, AgeProfileSummary, BaselineRange, StatusShares,
};
pub use aggregate::{
    dist, Aggregator, AggregatorOptions, BandSummary, Dist, DistComparison, FlagCounts,
    FlagRateComparison, FlagRates, FlagSummary, Hist, LevelCounts, MeanMedian, QualitySummary,
    RowFlag, RowSide, RunCounts, RunSummary, SessionRow, SweepAxis, SweepCell, SweepSummary,
    Transition, MIN_N_FOR_DECILES,
};
pub use evaluate::{
    changed_params, evaluate, quality_status, Evaluation, QualityStatus, ResearchInput,
    SessionInput, Unscorable,
};
pub use profile::{
    AnalysisProfile, AoiParams, FixationAlgorithm, GazeProcessingView, IvtUnit, MetricsParams,
    NormsParams, ReviewParams, DEFAULT_BLINK_MAX_SEC, DEFAULT_BLINK_MIN_SEC,
    DEFAULT_IDT_DISPERSION_NORM,
};
pub use suppress::suppress;
pub use sweep::{SweepAggregator, SweepError, SweepGrid};

/// Requested evaluation level (RPX-3; `asd_contract::research::LevelRequest`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LevelRequest {
    /// `frames` when the session has a research frame log, else `score`.
    #[default]
    Auto,
    Score,
    Frames,
}

/// The level a session was evaluated at (`asd_contract::research::Level`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Level {
    /// Metric sets rebuilt from the session's own score rows (`metrics::from_reports`).
    Score,
    /// Metric sets reprocessed from the research frame log (`gaze::reprocess`).
    Frames,
}
