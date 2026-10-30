//! owner: WP-R.1
//!
//! Green / yellow / red attention flag: port of `AttentionFlagAlgorithm.assess`
//! (`AttentionFlag.swift`). CORE-4.
//!
//! "No scored clips" is yellow, never `unreliable`: that value exists only in the
//! backend-derived column (CON-15, trap 6).

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

fn assessment(flag: AttentionFlag, headline: &str, detail: &str) -> AttentionFlagAssessment {
    AttentionFlagAssessment {
        flag,
        headline: headline.to_owned(),
        detail: detail.to_owned(),
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
        return assessment(
            AttentionFlag::Yellow,
            "Not enough scored clips",
            "Run a full stimulus set before reading a flag. This is a screening signal, not a diagnosis.",
        );
    }

    let low_count = scored.iter().filter(|v| v.low_confidence).count();
    let low_share = low_count as f64 / scored.len() as f64;
    if low_share >= thresholds.low_confidence_share {
        return assessment(
            AttentionFlag::Yellow,
            "Recheck — looking time was unreliable",
            "Too many clips fell below the quality gate. Seat the child again and repeat. This is a screening signal, not a diagnosis.",
        );
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
        return assessment(
            AttentionFlag::Green,
            "Looking matches the age reference",
            "Looking time sits in the expected range for this age. This is a screening signal, not a diagnosis.",
        );
    }

    if composite < thresholds.yellow_floor || atypical_share >= thresholds.atypical_share_for_red {
        return assessment(
            AttentionFlag::Red,
            "Looking differs from the age reference",
            "Share this result with your pediatrician at the next visit. This is a screening signal, not a diagnosis.",
        );
    }

    assessment(
        AttentionFlag::Yellow,
        "Some clips differ from the age reference",
        "A repeat session is worth doing. This is a screening signal, not a diagnosis.",
    )
}
