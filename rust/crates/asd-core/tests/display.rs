//! owner: WP-X.0c
//!
//! DSP-3 and DSP-4: the band decision and the session verdict on the default parameters give
//! exactly what the iOS views and `GazeProcessingActor` computed with literal cut-offs
//! (technical §5.1 #4-#20). The old Swift expressions are copied below as the oracle and
//! every cut-off is probed at, just below and just above.

// The oracle keeps the Swift expressions verbatim (`v >= 50 && v < 60`).
#![allow(clippy::manual_range_contains)]

use asd_core::display::{band, BandStatus, BandTarget, Comparator, DisplayMetric, DisplayParams};
use asd_core::quality::{session_verdict, SessionQualityParams};
use asd_core::types::GazeQualityVerdict;

use BandStatus::{Monitor, NotObserved, Ok, Outside};

/// The Swift expressions of §5.1 (after owner question 16: 60/50, 25/18, 550/750).
fn swift_oracle(metric: DisplayMetric, v: f64) -> BandStatus {
    match metric {
        // EngagementRiskStratificationView:160-161, EngagementDetailViews:392-393
        DisplayMetric::SocialPreferencePct => {
            if v >= 60.0 {
                Ok
            } else if v >= 50.0 && v < 60.0 {
                Monitor
            } else {
                Outside
            }
        }
        // EngagementDetailViews:482-483: (100 - socialPrefPct) <= 40, on the non-social value
        DisplayMetric::NonSocialPct => {
            if v <= 40.0 {
                Ok
            } else {
                Outside
            }
        }
        // EngagementRiskStratificationView:179-180, EngagementDetailViews:495-496
        DisplayMetric::EyeDwellPct => {
            if v >= 25.0 {
                Ok
            } else if v >= 18.0 && v < 25.0 {
                Monitor
            } else {
                Outside
            }
        }
        // EngagementDetailViews:508-509
        DisplayMetric::MouthDwellPct => {
            if v >= 10.0 && v <= 30.0 {
                Ok
            } else {
                Outside
            }
        }
        // EngagementRiskStratificationView:198-199
        DisplayMetric::FirstEyeLatencyMs => {
            if v < 550.0 {
                Ok
            } else if v >= 550.0 && v < 750.0 {
                Monitor
            } else {
                Outside
            }
        }
        // EngagementRiskStratificationView:217-219, EngagementAffectAndQualityViews:27-29
        DisplayMetric::SmileIntensityPct => {
            if v >= 20.0 {
                Ok
            } else if v >= 10.0 {
                Monitor
            } else {
                Outside
            }
        }
        // EngagementAffectAndQualityViews:116-117
        DisplayMetric::SmileReactivity => {
            if v >= 0.30 {
                Ok
            } else {
                Outside
            }
        }
        // EngagementAffectAndQualityViews:128-129
        DisplayMetric::SurprisePct => {
            if v >= 15.0 {
                Ok
            } else {
                Outside
            }
        }
        // EngagementAffectAndQualityViews:139-140
        DisplayMetric::FlatAffect => {
            if v <= 0.25 {
                Ok
            } else {
                Outside
            }
        }
        // EngagementRiskStratificationView:234, EngagementAffectAndQualityViews:179
        DisplayMetric::ValidFramePct => {
            if v >= 75.0 {
                Ok
            } else {
                Outside
            }
        }
    }
}

/// Every cut-off of §5.1 per metric, and the probe step around it.
fn cut_offs(metric: DisplayMetric) -> (&'static [f64], f64) {
    match metric {
        DisplayMetric::SocialPreferencePct => (&[50.0, 60.0], 0.01),
        DisplayMetric::NonSocialPct => (&[40.0], 0.01),
        DisplayMetric::EyeDwellPct => (&[18.0, 25.0], 0.01),
        DisplayMetric::MouthDwellPct => (&[10.0, 30.0], 0.01),
        DisplayMetric::FirstEyeLatencyMs => (&[550.0, 750.0], 0.5),
        DisplayMetric::SmileIntensityPct => (&[10.0, 20.0], 0.01),
        DisplayMetric::SmileReactivity => (&[0.30], 0.001),
        DisplayMetric::SurprisePct => (&[15.0], 0.01),
        DisplayMetric::FlatAffect => (&[0.25], 0.001),
        DisplayMetric::ValidFramePct => (&[75.0], 0.01),
    }
}

fn status(metric: DisplayMetric, v: Option<f64>) -> BandStatus {
    band(
        metric,
        v,
        &DisplayParams::default(),
        &SessionQualityParams::default(),
    )
    .0
}

#[test]
fn default_bands_match_the_swift_operators_at_every_cut_off() {
    let mut probes = 0;
    for metric in DisplayMetric::ALL {
        let (cuts, step) = cut_offs(metric);
        for &c in cuts {
            for v in [c - step, c, c + step] {
                assert_eq!(
                    status(metric, Some(v)),
                    swift_oracle(metric, v),
                    "{} at {v}",
                    metric.as_str()
                );
                probes += 1;
            }
        }
        // Far ends too.
        for v in [0.0, 1_000.0] {
            assert_eq!(status(metric, Some(v)), swift_oracle(metric, v));
        }
        assert_eq!(status(metric, None), NotObserved, "{}", metric.as_str());
    }
    // The 13 band leaves give 15 cut-offs (the non-social bound reuses socialPreferencePct.okMin,
    // the valid-frame bound is sessionQuality.optimalMinValidRatio x 100).
    assert_eq!(probes, 3 * 15, "15 cut-offs, three probes each");
}

#[test]
fn boundary_values_land_on_the_documented_side() {
    use DisplayMetric as M;
    let table: &[(M, f64, BandStatus)] = &[
        (M::SocialPreferencePct, 60.0, Ok),
        (M::SocialPreferencePct, 59.99, Monitor),
        (M::SocialPreferencePct, 50.0, Monitor),
        (M::SocialPreferencePct, 49.99, Outside),
        (M::NonSocialPct, 40.0, Ok),
        (M::NonSocialPct, 40.01, Outside),
        (M::EyeDwellPct, 25.0, Ok),
        (M::EyeDwellPct, 18.0, Monitor),
        (M::EyeDwellPct, 17.99, Outside),
        (M::MouthDwellPct, 10.0, Ok),
        (M::MouthDwellPct, 30.0, Ok),
        (M::MouthDwellPct, 30.01, Outside),
        (M::MouthDwellPct, 9.99, Outside),
        (M::FirstEyeLatencyMs, 549.9, Ok),
        (M::FirstEyeLatencyMs, 550.0, Monitor),
        (M::FirstEyeLatencyMs, 749.9, Monitor),
        (M::FirstEyeLatencyMs, 750.0, Outside),
        (M::SmileIntensityPct, 20.0, Ok),
        (M::SmileIntensityPct, 10.0, Monitor),
        (M::SmileIntensityPct, 9.99, Outside),
        (M::SmileReactivity, 0.30, Ok),
        (M::SmileReactivity, 0.299, Outside),
        (M::SurprisePct, 15.0, Ok),
        (M::SurprisePct, 14.99, Outside),
        (M::FlatAffect, 0.25, Ok),
        (M::FlatAffect, 0.251, Outside),
        (M::ValidFramePct, 75.0, Ok),
        (M::ValidFramePct, 74.99, Outside),
    ];
    for &(m, v, want) in table {
        assert_eq!(status(m, Some(v)), want, "{} at {v}", m.as_str());
    }
}

#[test]
fn targets_carry_the_comparator_bounds_and_unit() {
    let d = DisplayParams::default();
    let q = SessionQualityParams::default();
    let t = |m| band(m, None, &d, &q).1;
    assert_eq!(
        t(DisplayMetric::SocialPreferencePct),
        BandTarget {
            comparator: Comparator::AtLeast,
            low: Some(60.0),
            high: None,
            unit: "percent"
        }
    );
    assert_eq!(
        t(DisplayMetric::NonSocialPct),
        BandTarget {
            comparator: Comparator::AtMost,
            low: None,
            high: Some(40.0),
            unit: "percent"
        }
    );
    assert_eq!(
        t(DisplayMetric::MouthDwellPct),
        BandTarget {
            comparator: Comparator::Between,
            low: Some(10.0),
            high: Some(30.0),
            unit: "percent"
        }
    );
    assert_eq!(
        t(DisplayMetric::FirstEyeLatencyMs),
        BandTarget {
            comparator: Comparator::Below,
            low: None,
            high: Some(550.0),
            unit: "millis"
        }
    );
    assert_eq!(t(DisplayMetric::FlatAffect).comparator, Comparator::AtMost);
    assert_eq!(t(DisplayMetric::SmileReactivity).unit, "ratio");
    assert_eq!(t(DisplayMetric::ValidFramePct).low, Some(75.0));
}

#[test]
fn parameters_move_the_cut_offs() {
    // Acceptance 15: eyeDwellPct.okMin = 30 turns 27 % from ok to monitor.
    let mut d = DisplayParams::default();
    let q = SessionQualityParams::default();
    assert_eq!(band(DisplayMetric::EyeDwellPct, Some(27.0), &d, &q).0, Ok);
    d.eye_dwell_pct_ok_min = 30.0;
    let (s, t) = band(DisplayMetric::EyeDwellPct, Some(27.0), &d, &q);
    assert_eq!((s, t.low), (Monitor, Some(30.0)));
    // Non-social follows socialPreferencePct.okMin.
    d.social_preference_pct_ok_min = 70.0;
    assert_eq!(
        band(DisplayMetric::NonSocialPct, Some(35.0), &d, &q).0,
        Outside
    );
    assert_eq!(band(DisplayMetric::NonSocialPct, Some(30.0), &d, &q).0, Ok);
    // Valid frames follow sessionQuality.optimalMinValidRatio.
    let q = SessionQualityParams {
        optimal_min_valid_ratio: 0.5,
        ..Default::default()
    };
    assert_eq!(band(DisplayMetric::ValidFramePct, Some(60.0), &d, &q).0, Ok);
}

/// DSP-4: `GazeProcessingActor.swift:1175-1178` copied as the oracle.
fn swift_verdict(valid_ratio: f64, interrupted: bool) -> GazeQualityVerdict {
    let mut verdict = if valid_ratio >= 0.75 {
        GazeQualityVerdict::Optimal
    } else if valid_ratio >= 0.50 {
        GazeQualityVerdict::Acceptable
    } else {
        GazeQualityVerdict::Degraded
    };
    if interrupted
        && (verdict == GazeQualityVerdict::Optimal || verdict == GazeQualityVerdict::Acceptable)
    {
        verdict = GazeQualityVerdict::Degraded;
    }
    verdict
}

#[test]
fn session_verdict_matches_the_swift_actor() {
    use GazeQualityVerdict::{Acceptable, Degraded, Optimal};
    let p = SessionQualityParams::default();
    let table = [
        (0.75, false, Optimal),
        (0.749, false, Acceptable),
        (0.50, false, Acceptable),
        (0.499, false, Degraded),
        (0.75, true, Degraded),
        (0.749, true, Degraded),
        (0.50, true, Degraded),
        (0.499, true, Degraded),
    ];
    for (ratio, interrupted, want) in table {
        assert_eq!(
            session_verdict(ratio, interrupted, &p),
            want,
            "{ratio} {interrupted}"
        );
        assert_eq!(swift_verdict(ratio, interrupted), want);
    }
    assert_eq!(session_verdict(f64::NAN, false, &p), Degraded);
    // Acceptance 15: optimal at 0.85 stores a 0.80 session as acceptable.
    let p = SessionQualityParams {
        optimal_min_valid_ratio: 0.85,
        ..Default::default()
    };
    assert_eq!(session_verdict(0.80, false, &p), Acceptable);
}
