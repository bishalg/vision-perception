//! owner: WP-R.1
//!
//! Green / yellow / red attention flag: port of `AttentionFlagAlgorithm.assess`
//! (`AttentionFlag.swift`). CORE-4.
//!
//! "No scored clips" is yellow, never `unreliable`: that value exists only in the
//! backend-derived column (CON-15, trap 6).
//!
//! I18N-4 (WP-L.0): the five caregiver headline/detail pairs are [`FlagCase`] constants. The
//! English text is written into the session as before (byte for byte); surfaces map a sent
//! headline back to its case with [`flag_case`] and render `flag_assessment.<case>.*` from
//! the `asd-contract` catalogue, whose `en` values a test pins to these constants.

use serde::{Deserialize, Serialize};

use crate::types::{
    AttentionFlag, AttentionFlagAssessment, ExpectationStatus, PerVideoSessionScore,
};

/// Flag thresholds. Serialised with the `ScoringProfile.flag` key names of CON-5 so the
/// contract crate can embed this struct; defaults are the Swift constants.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FlagThresholds {
    /// Composite at or above this (with few atypical rows) is green.
    pub green_floor: f64,
    /// Composite below this is red.
    pub yellow_floor: f64,
    /// Share of scored clips below the quality gate that forces a recheck (yellow).
    pub low_confidence_share: f64,
    /// Share of scored rows beyond the reference band that forces red.
    pub atypical_share_for_red: f64,
}

impl Default for FlagThresholds {
    fn default() -> Self {
        Self {
            green_floor: 0.65,
            yellow_floor: 0.35,
            low_confidence_share: 0.50,
            atypical_share_for_red: 0.40,
        }
    }
}

/// The five outcomes of [`assess`], each with one fixed English headline and detail.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FlagCase {
    NoScoredClips,
    Unreliable,
    Matches,
    Differs,
    SomeDiffer,
}

impl FlagCase {
    /// Every case, in the order [`assess`] tests them.
    pub const ALL: [FlagCase; 5] = [
        FlagCase::NoScoredClips,
        FlagCase::Unreliable,
        FlagCase::Matches,
        FlagCase::Differs,
        FlagCase::SomeDiffer,
    ];

    /// Stable id (`no_scored_clips`, ...): the catalogue key segment and the wasm
    /// `flagCase` result.
    pub const fn id(self) -> &'static str {
        match self {
            Self::NoScoredClips => "no_scored_clips",
            Self::Unreliable => "unreliable",
            Self::Matches => "matches",
            Self::Differs => "differs",
            Self::SomeDiffer => "some_differ",
        }
    }

    /// Catalogue key prefix `flag_assessment.<id>`; `.headline` and `.detail` follow.
    pub const fn key(self) -> &'static str {
        match self {
            Self::NoScoredClips => "flag_assessment.no_scored_clips",
            Self::Unreliable => "flag_assessment.unreliable",
            Self::Matches => "flag_assessment.matches",
            Self::Differs => "flag_assessment.differs",
            Self::SomeDiffer => "flag_assessment.some_differ",
        }
    }

    /// The flag [`assess`] gives with this case.
    pub const fn flag(self) -> AttentionFlag {
        match self {
            Self::Matches => AttentionFlag::Green,
            Self::Differs => AttentionFlag::Red,
            Self::NoScoredClips | Self::Unreliable | Self::SomeDiffer => AttentionFlag::Yellow,
        }
    }

    /// English headline written into the session.
    pub const fn headline(self) -> &'static str {
        match self {
            Self::NoScoredClips => "Not enough scored clips",
            Self::Unreliable => "Recheck — looking time was unreliable",
            Self::Matches => "Looking matches the age reference",
            Self::Differs => "Looking differs from the age reference",
            Self::SomeDiffer => "Some clips differ from the age reference",
        }
    }

    /// English detail written into the session.
    pub const fn detail(self) -> &'static str {
        match self {
            Self::NoScoredClips => {
                "Run a full stimulus set before reading a flag. This is a screening signal, not a diagnosis."
            }
            Self::Unreliable => {
                "Too many clips fell below the quality gate. Seat the child again and repeat. This is a screening signal, not a diagnosis."
            }
            Self::Matches => {
                "Looking time sits in the expected range for this age. This is a screening signal, not a diagnosis."
            }
            Self::Differs => {
                "Share this result with your pediatrician at the next visit. This is a screening signal, not a diagnosis."
            }
            Self::SomeDiffer => {
                "A repeat session is worth doing. This is a screening signal, not a diagnosis."
            }
        }
    }
}

/// The case of an assessment, matched on its English headline; `None` for a headline this
/// version does not know (text written by another core version).
pub fn flag_case(a: &AttentionFlagAssessment) -> Option<FlagCase> {
    flag_case_of_headline(&a.headline)
}

/// [`flag_case`] on a bare headline (the wasm `flagCase` export).
pub fn flag_case_of_headline(headline: &str) -> Option<FlagCase> {
    FlagCase::ALL.into_iter().find(|c| c.headline() == headline)
}

fn assessment(case: FlagCase) -> AttentionFlagAssessment {
    AttentionFlagAssessment {
        flag: case.flag(),
        headline: case.headline().to_owned(),
        detail: case.detail().to_owned(),
        is_screening_signal_only: true,
    }
}

/// Rolls a per-stimulus session score into one flag with the caregiver-facing strings.
pub fn assess(
    score: &PerVideoSessionScore,
    thresholds: &FlagThresholds,
) -> AttentionFlagAssessment {
    let scored: Vec<_> = score.videos.iter().filter(|v| v.has_baseline).collect();
    if scored.is_empty() {
        return assessment(FlagCase::NoScoredClips);
    }

    let low_count = scored.iter().filter(|v| v.low_confidence).count();
    let low_share = low_count as f64 / scored.len() as f64;
    if low_share >= thresholds.low_confidence_share {
        return assessment(FlagCase::Unreliable);
    }

    let rows: Vec<_> = scored.iter().flat_map(|v| v.rows.iter()).collect();
    let atypical_share = if rows.is_empty() {
        0.0
    } else {
        let atypical = rows
            .iter()
            .filter(|r| r.status == ExpectationStatus::Atypical)
            .count();
        atypical as f64 / rows.len() as f64
    };

    let composite = score.composite_score;
    if composite >= thresholds.green_floor && atypical_share < thresholds.atypical_share_for_red {
        return assessment(FlagCase::Matches);
    }

    if composite < thresholds.yellow_floor || atypical_share >= thresholds.atypical_share_for_red {
        return assessment(FlagCase::Differs);
    }

    assessment(FlagCase::SomeDiffer)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_case_round_trips_through_its_headline() {
        for case in FlagCase::ALL {
            let a = assessment(case);
            assert_eq!(flag_case(&a), Some(case));
            assert_eq!(a.flag, case.flag());
            assert!(case.key().ends_with(case.id()));
        }
        assert_eq!(flag_case_of_headline("Looking matches"), None);
    }
}
