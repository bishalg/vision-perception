//! owner: WP-R.0
//!
//! Serialisable domain types (CON-1): 1:1 ports of the Swift `Codable` structs in
//! `domains/asd_screening/ios/Sources/CurvAsdGazeKit` with identical JSON keys.
//!
//! Rules for every type here:
//! - data only, no logic (logic lives in the sibling modules);
//! - `deny_unknown_fields` is never set (Swift tolerates extra keys);
//! - an optional Swift field is `Option<T>` with `skip_serializing_if = "Option::is_none"`,
//!   matching `encodeIfPresent`;
//! - maps are `BTreeMap` so iteration order is deterministic.

pub mod baselines;
pub mod capture;
pub mod catalog;
pub mod flag;
pub mod frame;
pub mod metrics;
pub mod quality;
pub mod scoring;
pub mod tracking;

pub use baselines::{
    AgeBracket, NormativeBaselineSet, NormativeDirection, NormativeExpectation, NormativeRange,
    NormativeSegment, NormativeVideo,
};
pub use capture::CaptureSource;
pub use catalog::{AgePack, Area, Clip, Membership, StimulusCatalog};
pub use flag::{AttentionFlag, AttentionFlagAssessment};
pub use frame::{GazeFrame, GazeFrameAffect, GazeFrameSample, GazeTelemetrySample};
pub use metrics::PerVideoMetricSet;
pub use quality::GazeQualityVerdict;
pub use scoring::{ExpectationStatus, PerVideoScoreReport, PerVideoScoreRow, PerVideoSessionScore};
