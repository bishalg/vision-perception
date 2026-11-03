//! owner: WP-X.9
//!
//! SWP-1..3, AGE-1, AGE-3, PRV-1 on the phase-B analysis seams:
//!
//! - the sweep grid (clean values, the published value always a cell, the nearest cell to the
//!   experiment), registry ids written into a profile, and per-cell aggregation of real
//!   evaluations (`changed_vs_published` is 0 in the published cell);
//! - the age profile's shares and expected percentile on a fixture with known answers, and a
//!   draft baselines set that moves the shares the way its ranges say;
//! - suppression per sweep cell and per age band / cell with `max(k, norms.minN)`;
//! - a summary carrying both blocks decodes as the contract's `RunSummary`.

use asd_core::analysis::sweep::{
    axis_values, grid_cells, param_value, with_values, worst_case_bytes,
};
use asd_core::analysis::{
    evaluate, suppress, AgeProfileAggregator, AgeProfileBaselines, AgeProfileSummary,
    AggregatorOptions, AnalysisProfile, Evaluation, Level, LevelRequest, QualityStatus, RowSide,
    RunSummary, SessionInput, SweepAggregator, SweepAxis, SweepGrid,
};
use asd_core::norms::default_age_brackets;
use asd_core::types::{
    ExpectationStatus, NormativeBaselineSet, NormativeDirection, PerVideoScoreReport,
    PerVideoScoreRow, PerVideoSessionScore,
};
use serde_json::Value;

const BASELINES: &str = include_str!("fixtures/normative_baselines.json");

fn baselines() -> NormativeBaselineSet {
    serde_json::from_str(BASELINES).unwrap()
}

fn axis(id: &str, from: f64, to: f64, step: f64) -> SweepAxis {
    SweepAxis {
        param_id: id.to_owned(),
        from,
        to,
        step,
    }
}

/// A synth session (`asd-cli synth`) as the evaluation reads it.
fn synth_input(index: u64) -> SessionInput {
    let s = asd_core::synth::generate_session(7, index, None);
    let raw = &s["rawPayload"];
    SessionInput {
        session_id: format!("S-{index}"),
        age_months: raw["ageMonths"].as_i64(),
        reports: serde_json::from_value::<Vec<PerVideoScoreReport>>(raw["perVideo"].clone())
            .unwrap(),
        has_reports: true,
        valid_frame_ratio: raw["validFrameRatio"].as_f64(),
        tracking: None,
        scored_at_utc: String::new(),
        research: None,
    }
}

// ---- SWP-1 / SWP-2: the grid ----------------------------------------------------------------

#[test]
fn axis_values_are_clean_and_end_at_or_before_to() {
    let a = axis("flag.greenFloor", 0.5, 0.8, 0.1);
    assert_eq!(axis_values(&a).unwrap(), vec![0.5, 0.6, 0.7, 0.8]);
    let b = axis("flag.greenFloor", 0.5, 0.82, 0.05);
    assert_eq!(
        axis_values(&b).unwrap(),
        vec![0.5, 0.55, 0.6, 0.65, 0.7, 0.75, 0.8]
    );
    let c = axis("norms.minN", 5.0, 50.0, 5.0);
    assert_eq!(axis_values(&c).unwrap().len(), 10);
    assert!(axis_values(&axis("x", 1.0, 0.0, 0.1)).is_err());
    assert!(axis_values(&axis("x", 0.0, 1.0, 0.0)).is_err());
    assert_eq!(
        grid_cells(&[axis("a", 0.0, 2.4, 0.1), axis("b", 0.0, 24.0, 1.0)]).unwrap(),
        625
    );
}

#[test]
fn the_grid_includes_the_published_value() {
    let axes = [axis("flag.greenFloor", 0.5, 0.8, 0.1)];
    // On the grid: no extra cell.
    let g = SweepGrid::new(&axes, &[0.6], &[0.72]).unwrap();
    assert_eq!(g.cells.len(), 4);
    assert_eq!(g.cells[g.published_cell], vec![0.6]);
    assert_eq!(
        g.cells[g.nearest_cell],
        vec![0.7],
        "nearest to the experiment"
    );
    // Off the grid (0.65): appended as the last cell; outside [from, to] too.
    let g = SweepGrid::new(&axes, &[0.65], &[0.65]).unwrap();
    assert_eq!(g.cells.len(), 5);
    assert_eq!(g.published_cell, 4);
    assert_eq!(g.cells[4], vec![0.65]);
    assert_eq!(
        g.cells[g.nearest_cell],
        vec![0.6],
        "a tie goes to the lower value"
    );
    let g = SweepGrid::new(&axes, &[0.95], &[0.5]).unwrap();
    assert_eq!(g.cells.last().unwrap(), &vec![0.95]);

    // Two axes, 25 x 25 = 625 grid cells plus the published point.
    let axes = [
        axis("flag.greenFloor", 0.4, 0.88, 0.02),
        axis("flag.yellowFloor", 0.1, 0.58, 0.02),
    ];
    let g = SweepGrid::new(&axes, &[0.65, 0.35], &[0.65, 0.35]).unwrap();
    assert_eq!(g.cells.len(), 626);
    assert_eq!(g.cells[g.published_cell], vec![0.65, 0.35]);
    assert_eq!(g.cells[1], vec![0.4, 0.12], "row-major, last axis fastest");
    let g = SweepGrid::new(&axes, &[0.64, 0.34], &[0.64, 0.34]).unwrap();
    assert_eq!(g.cells.len(), 625);
    assert_eq!(g.published_cell, g.nearest_cell);
    assert!(SweepGrid::new(&[], &[], &[]).is_err());
}

#[test]
fn registry_ids_read_and_write_the_profile() {
    let p = AnalysisProfile::default();
    assert_eq!(param_value(&p, "flag.greenFloor"), Some(p.flag.green_floor));
    assert_eq!(
        param_value(&p, "norms.minN"),
        Some(f64::from(p.norms.min_n))
    );
    assert_eq!(
        param_value(&p, "trackingQuality.scoring.maxAccuracyDegForScoring"),
        Some(p.tracking_quality.scoring.max_accuracy_deg_for_scoring),
        "owner-gated (H-G2) knobs are sweepable in analysis (Q9)"
    );
    assert_eq!(
        param_value(&p, "metrics.ivtUnit"),
        None,
        "an enum is not a number"
    );
    assert_eq!(param_value(&p, "no.such.id"), None);

    let axes = [
        axis("flag.greenFloor", 0.0, 1.0, 0.1),
        axis("norms.minN", 5.0, 50.0, 1.0),
    ];
    let q = with_values(&p, &axes, &[0.3, 20.0]).unwrap();
    assert_eq!(q.flag.green_floor, 0.3);
    assert_eq!(q.norms.min_n, 20);
    assert_eq!(q.flag.yellow_floor, p.flag.yellow_floor, "the rest stays");
    assert!(
        with_values(&p, &axes, &[0.3, 20.5]).is_err(),
        "an integer knob"
    );
    assert!(with_values(&p, &[axis("no.such", 0.0, 1.0, 1.0)], &[0.0]).is_err());
}

// ---- SWP-3: per-cell aggregation of real evaluations -----------------------------------------

#[test]
fn sweep_cells_aggregate_evaluations_and_the_published_cell_changes_nothing() {
    let published = AnalysisProfile::default();
    let b = baselines();
    let axes = [axis("flag.greenFloor", 0.0, 1.0, 0.25)];
    let grid = SweepGrid::new(
        &axes,
        &[published.flag.green_floor],
        &[published.flag.green_floor],
    )
    .unwrap();
    assert_eq!(
        grid.cells.len(),
        6,
        "0, .25, .5, .75, 1 and the published .65"
    );
    let profiles = grid.profiles(&published).unwrap();
    let keys = vec!["eye_dwell_ratio".to_owned()];
    let mut agg = SweepAggregator::new(&grid, &keys);
    let sessions: Vec<SessionInput> = (0..24).map(synth_input).collect();
    for s in &sessions {
        let p = RowSide::from_evaluation(&evaluate(s, &published, &b, LevelRequest::Score));
        for (i, profile) in profiles.iter().enumerate() {
            let e = evaluate(s, profile, &b, LevelRequest::Score);
            agg.push(i, &p, &RowSide::from_evaluation(&e));
        }
    }
    let out = agg.finish();
    assert_eq!(out.published_values, vec![published.flag.green_floor]);
    assert_eq!(out.cells.len(), 6);
    let pub_cell = &out.cells[grid.published_cell];
    assert_eq!(pub_cell.changed_vs_published, Some(0));
    for c in &out.cells {
        assert_eq!(c.n, Some(24));
        let r = c.flag_rate.unwrap();
        assert!((r.green + r.yellow + r.red + r.unreliable - 1.0).abs() < 1e-12);
        let q: f64 = c.quality_rate.values().map(|v| v.unwrap()).sum();
        assert!((q - 1.0).abs() < 1e-12);
        assert!(c.metrics["eye_dwell_ratio"].mean.is_some());
        // The green floor moves the flag, never the composite.
        assert_eq!(c.composite, pub_cell.composite);
    }
    // A floor of 0 makes every scored session green; the green share never rises with it.
    let green: Vec<f64> = out.cells[..5]
        .iter()
        .map(|c| c.flag_rate.unwrap().green)
        .collect();
    assert!(green.windows(2).all(|w| w[0] >= w[1]), "{green:?}");
    assert!(out.cells[0].changed_vs_published.unwrap() > 0);
}

#[test]
fn the_summary_estimate_bounds_a_625_cell_sweep() {
    let all: Vec<String> = asd_core::metrics::PerVideoMetricKey::ALL
        .iter()
        .map(|k| k.as_str().to_owned())
        .collect();
    let composite_only = worst_case_bytes(2, 626, &[]);
    assert!(composite_only < 512 * 1024, "{composite_only}");
    // Every metric in every cell of the largest grid would not fit the 1 MiB summary (RUN-4):
    // the cells summarise the chosen metrics only, and the route checks the estimate.
    let every_metric = worst_case_bytes(2, 626, &all);
    assert!(every_metric > 1024 * 1024, "{every_metric}");
}

// ---- AGE-1: shares and percentiles on a fixture with known answers ----------------------------

fn score_row(
    observed: f64,
    expected: f64,
    low: f64,
    status: ExpectationStatus,
) -> PerVideoScoreRow {
    PerVideoScoreRow {
        segment_id: "full".into(),
        segment_description: String::new(),
        biomarker: "eye_dwell_ratio".into(),
        target_aoi: None,
        metric_key: "eye_dwell_ratio".into(),
        direction: NormativeDirection::HigherBetter,
        observed,
        expected: Some(expected),
        acceptable_low: Some(low),
        acceptable_high: None,
        status,
        score: 1.0,
        imputed: false,
        citation: String::new(),
        source: "literature".into(),
    }
}

/// An evaluation with one scored row (`higherBetter`: on target at `>= expected`, monitor at
/// `>= low`, else atypical), classified as the scoring does.
fn evaluation(observed: f64, expected: f64, low: f64) -> Evaluation {
    let status = if observed >= expected {
        ExpectationStatus::OnTarget
    } else if observed >= low {
        ExpectationStatus::Monitor
    } else {
        ExpectationStatus::Atypical
    };
    Evaluation {
        level: Level::Score,
        unscorable: None,
        score: Some(PerVideoSessionScore {
            age_bracket_id: "24-35".into(),
            age_months: 30,
            composite_score: 0.5,
            videos: vec![PerVideoScoreReport {
                stimulus_id: "clip_a".into(),
                title: String::new(),
                category: String::new(),
                rows: vec![score_row(observed, expected, low, status)],
                subscore: 0.5,
                valid_sample_ratio: 1.0,
                low_confidence: false,
                has_baseline: true,
                weight: 1.0,
            }],
            baseline_status: String::new(),
            baseline_schema_version: String::new(),
            scored_at_utc: String::new(),
        }),
        flag: None,
        quality_status: QualityStatus::Good,
        valid_ratio: Some(1.0),
        metrics: Vec::new(),
        tracking: None,
        fixations: Vec::new(),
        trace: None,
        warnings: Vec::new(),
    }
}

fn versions() -> AgeProfileBaselines {
    AgeProfileBaselines {
        published: "pub".into(),
        draft: "draft".into(),
    }
}

#[test]
fn age_profile_shares_and_percentiles_have_known_answers() {
    let mut agg =
        AgeProfileAggregator::new(&default_age_brackets(), false, AggregatorOptions::default());
    // 20 sessions aged 30 (band 24-35), observed 0..19. Published range: expected 10, low 5
    // -> 10 on target, 5 monitor, 5 atypical; expected 10 sits at mid-rank (10 + 0.5) / 20.
    // Draft range: expected 15, low 10 -> 5 / 5 / 10.
    for v in 0..20 {
        let v = f64::from(v);
        agg.push(
            Some(30),
            &evaluation(v, 10.0, 5.0),
            &evaluation(v, 15.0, 10.0),
        );
    }
    agg.push(
        None,
        &evaluation(1.0, 10.0, 5.0),
        &evaluation(1.0, 10.0, 5.0),
    );
    let out = agg.finish(versions(), 12, 7);
    let band = out.bands.iter().find(|b| b.band == "24-35").unwrap();
    assert_eq!(
        band.n,
        Some(20),
        "the session without an age is not counted"
    );
    assert_eq!(band.expectations.len(), 1);
    let e = &band.expectations[0];
    assert_eq!(
        (e.stimulus_id.as_str(), e.metric_key.as_str()),
        ("clip_a", "eye_dwell_ratio")
    );
    let p = &e.published;
    assert_eq!(p.n, Some(20));
    let s = p.shares.unwrap();
    assert_eq!((s.on_target, s.monitor, s.atypical), (0.5, 0.25, 0.25));
    assert_eq!(p.expected_percentile, Some(52.5));
    assert_eq!(p.range.unwrap().expected, Some(10.0));
    assert_eq!(p.dist.median, Some(9.5));
    // The draft baselines move the shares as their range says.
    let d = &e.draft;
    let s = d.shares.unwrap();
    assert_eq!((s.on_target, s.monitor, s.atypical), (0.25, 0.25, 0.5));
    assert_eq!(d.expected_percentile, Some(77.5));
    // Other bands exist, empty.
    assert!(out
        .bands
        .iter()
        .all(|b| b.band == "24-35" || b.n == Some(0)));

    // Rows that disagree on the range give no range and no percentile.
    let mut agg =
        AgeProfileAggregator::new(&default_age_brackets(), false, AggregatorOptions::default());
    agg.push(
        Some(30),
        &evaluation(1.0, 10.0, 5.0),
        &evaluation(1.0, 10.0, 5.0),
    );
    agg.push(
        Some(30),
        &evaluation(1.0, 11.0, 5.0),
        &evaluation(1.0, 10.0, 5.0),
    );
    let out = agg.finish(versions(), 12, 7);
    let e = &out.bands[2].expectations[0];
    assert_eq!(e.published.range, None);
    assert_eq!(e.published.expected_percentile, None);
    assert!(e.draft.range.is_some());
}

#[test]
fn imputed_rows_follow_norms_include_imputed() {
    let mut imputed = evaluation(3.0, 10.0, 5.0);
    imputed.score.as_mut().unwrap().videos[0].rows[0].imputed = true;
    for (include, n) in [(false, 0), (true, 1)] {
        let mut agg = AgeProfileAggregator::new(
            &default_age_brackets(),
            include,
            AggregatorOptions::default(),
        );
        agg.push(Some(30), &imputed, &imputed);
        let out = agg.finish(versions(), 12, 7);
        let got = out.bands[2]
            .expectations
            .first()
            .and_then(|e| e.published.n)
            .unwrap_or(0);
        assert_eq!(got, n);
    }
}

#[test]
fn a_draft_baselines_set_moves_real_shares() {
    let profile = AnalysisProfile::default();
    let published = baselines();
    // Draft: every expectation's acceptable range lifted far above any observation, so every
    // `higherBetter` row becomes atypical.
    let mut raw: Value = serde_json::from_str(BASELINES).unwrap();
    for video in raw["videos"].as_object_mut().unwrap().values_mut() {
        for seg in video["segments"].as_array_mut().unwrap() {
            for e in seg["expectations"].as_array_mut().unwrap() {
                if e["direction"] == "higherBetter" {
                    for range in e["byAge"].as_object_mut().unwrap().values_mut() {
                        range["expected"] = 1e6.into();
                        range["acceptableLow"] = 1e5.into();
                        range.as_object_mut().unwrap().remove("acceptableHigh");
                    }
                }
            }
        }
    }
    let draft: NormativeBaselineSet = serde_json::from_value(raw).unwrap();
    let mut agg = AgeProfileAggregator::new(&profile.age_bands, true, AggregatorOptions::default());
    for i in 0..40 {
        let s = synth_input(i);
        let p = evaluate(&s, &profile, &published, LevelRequest::Score);
        let d = evaluate(&s, &profile, &draft, LevelRequest::Score);
        agg.push(s.age_months, &p, &d);
    }
    let out = agg.finish(versions(), profile.norms.min_n, 1);
    let mut moved = 0;
    for band in &out.bands {
        for e in &band.expectations {
            if e.direction != NormativeDirection::HigherBetter {
                continue;
            }
            let (Some(p), Some(d)) = (e.published.shares, e.draft.shares) else {
                continue;
            };
            assert_eq!(d.atypical, 1.0, "{e:?}");
            assert_eq!(e.draft.expected_percentile, Some(100.0));
            // The observations are the same profile's: only the classification moved.
            assert_eq!(e.published.dist.n, e.draft.dist.n);
            assert_eq!(e.published.dist.median, e.draft.dist.median);
            if p.atypical < 1.0 {
                moved += 1;
            }
        }
    }
    assert!(moved > 0, "some published cell was not all atypical");
}

// ---- PRV-1 / AGE-3: suppression ---------------------------------------------------------------

#[test]
fn suppression_per_band_and_per_cell() {
    let mut agg =
        AgeProfileAggregator::new(&default_age_brackets(), false, AggregatorOptions::default());
    for v in 0..13 {
        agg.push(
            Some(30),
            &evaluation(f64::from(v), 10.0, 5.0),
            &evaluation(f64::from(v), 10.0, 5.0),
        );
    }
    for v in 0..20 {
        let ev = evaluation(f64::from(v), 10.0, 5.0);
        agg.push(Some(40), &ev, &ev);
    }
    // norms.minN 15 > k 12: the band of 13 is suppressed.
    let block = agg.finish(versions(), 15, 3);
    let mut summary = RunSummary {
        age_profile: Some(serde_json::to_value(&block).unwrap()),
        ..RunSummary::default()
    };
    suppress(&mut summary, 12);
    let a: AgeProfileSummary =
        serde_json::from_value(summary.age_profile.clone().unwrap()).unwrap();
    assert_eq!(a.suppression_threshold, 15);
    let small = &a.bands[2];
    assert!(small.suppressed && small.n.is_none());
    let e = &small.expectations[0];
    assert!(e.published.suppressed && e.published.shares.is_none());
    assert!(e.published.dist.suppressed && e.published.dist.mean.is_none());
    assert!(e.published.expected_percentile.is_none());
    assert!(
        e.published.range.is_some(),
        "the baseline range is not cohort data"
    );
    let big = &a.bands[3];
    assert_eq!(big.n, Some(20));
    assert!(big.expectations[0].published.shares.is_some());
    // Idempotent.
    let once = summary.clone();
    suppress(&mut summary, 12);
    assert_eq!(summary, once);
    // A block that does not decode is not stored unchecked.
    let mut bad = RunSummary {
        age_profile: Some(serde_json::json!({"bands": "x"})),
        ..RunSummary::default()
    };
    suppress(&mut bad, 12);
    assert!(bad.age_profile.is_none());
}

#[test]
fn sweep_cells_are_suppressed_per_cell() {
    let axes = [axis("flag.greenFloor", 0.5, 0.6, 0.1)];
    let grid = SweepGrid::new(&axes, &[0.5], &[0.5]).unwrap();
    let mut agg = SweepAggregator::new(&grid, &[]);
    let side = |flag| RowSide {
        composite: Some(0.5),
        flag: Some(flag),
        quality_status: Some(QualityStatus::Good),
        metrics: Default::default(),
    };
    use asd_core::analysis::RowFlag::{Green, Yellow};
    for i in 0..20 {
        // Cell 0: 20 sessions, 3 changed flag; cell 1: 5 sessions.
        agg.push(0, &side(Green), &side(if i < 3 { Yellow } else { Green }));
        if i < 5 {
            agg.push(1, &side(Green), &side(Green));
        }
    }
    let mut summary = RunSummary {
        sweep: Some(agg.finish()),
        ..RunSummary::default()
    };
    suppress(&mut summary, 12);
    let s = summary.sweep.as_ref().unwrap();
    assert_eq!(s.cells[0].n, Some(20));
    assert_eq!(s.cells[0].changed_vs_published, None, "3 < k");
    assert_eq!(s.cells[1].n, None);
    assert_eq!(s.cells[1].values, vec![0.6]);
    assert!(s.cells[1].flag_rate.is_none() && s.cells[1].composite.mean.is_none());
    assert_eq!(s.published_values, vec![0.5]);
    assert_eq!(summary.counts.suppressed_cells, 1);
}

#[test]
fn a_summary_with_both_blocks_decodes_as_the_contract_type() {
    let axes = [axis("flag.greenFloor", 0.5, 0.6, 0.1)];
    let grid = SweepGrid::new(&axes, &[0.65], &[0.5]).unwrap();
    let mut sweep = SweepAggregator::new(&grid, &["eye_dwell_ratio".to_owned()]);
    let mut age =
        AgeProfileAggregator::new(&default_age_brackets(), false, AggregatorOptions::default());
    for v in 0..30 {
        let ev = evaluation(f64::from(v), 10.0, 5.0);
        age.push(Some(30), &ev, &ev);
        for c in 0..grid.cells.len() {
            let side = RowSide::from_evaluation(&ev);
            sweep.push(c, &side, &side);
        }
    }
    let mut s = RunSummary {
        sweep: Some(sweep.finish()),
        age_profile: Some(serde_json::to_value(age.finish(versions(), 12, 1)).unwrap()),
        ..RunSummary::default()
    };
    suppress(&mut s, 12);
    let core = serde_json::to_value(&s).unwrap();
    let contract: asd_contract::research::RunSummary =
        serde_json::from_value(core.clone()).expect("decodes as the contract RunSummary");
    assert_eq!(serde_json::to_value(&contract).unwrap(), core);
    assert_eq!(contract.sweep.unwrap().published_values, vec![0.65]);
}
