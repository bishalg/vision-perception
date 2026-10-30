//! owner: WP-R.4
//!
//! Ports the cases of `tests/test_asd_norm_aggregator.py` (the CLI end-to-end case lives in
//! `asd-cli/src/cmd/norms.rs`) plus percentile vectors and a golden candidate document.

use asd_core::norms::{
    aggregate, bracket_id_for, build_candidate_baseline, default_age_brackets,
    extract_observations, load_session_records, percentile, qualifying_cells, AggregateParams,
    Buckets,
};
use asd_core::types::NormativeDirection;
use serde_json::{json, Value};

fn row(biomarker: &str, observed: f64, imputed: bool) -> Value {
    json!({"segmentId": "s", "biomarker": biomarker, "observed": observed, "imputed": imputed})
}

fn score_record(age: i64, videos: Vec<(&str, Vec<Value>)>) -> Value {
    let brackets = default_age_brackets();
    json!({
        "type": "per_video_score",
        "score": {
            "ageBracketId": bracket_id_for(age, &brackets),
            "ageMonths": age,
            "compositeScore": 0.7,
            "baselineStatus": "provisional-literature-seeded",
            "baselineSchemaVersion": "1.0.0",
            "scoredAtUtc": "2026-09-28T00:00:00Z",
            "videos": videos.into_iter().map(|(id, rows)| json!({
                "stimulusId": id, "title": id, "subscore": 0.8, "hasBaseline": true,
                "lowConfidence": false, "validSampleRatio": 0.9, "rows": rows,
            })).collect::<Vec<_>>(),
        }
    })
}

fn extract(records: &[Value]) -> Buckets {
    let mut b = Buckets::new();
    extract_observations(records, &AggregateParams::default(), &mut b);
    b
}

fn key(s: &str, b: &str, a: &str) -> (String, String, String) {
    (s.into(), b.into(), a.into())
}

#[test]
fn bracket_inside() {
    let b = default_age_brackets();
    assert_eq!(bracket_id_for(20, &b).as_deref(), Some("18-23"));
    assert_eq!(bracket_id_for(48, &b).as_deref(), Some("48-60"));
}

#[test]
fn bracket_outside_returns_none() {
    let b = default_age_brackets();
    assert_eq!(bracket_id_for(6, &b), None);
    assert_eq!(bracket_id_for(72, &b), None);
}

#[test]
fn percentile_exact_indices() {
    assert_eq!(percentile(&[1.0, 2.0, 3.0, 4.0], 0.5), 2.5);
    assert_eq!(percentile(&[1.0, 2.0, 3.0], 0.0), 1.0);
    assert_eq!(percentile(&[1.0, 2.0, 3.0], 1.0), 3.0);
}

#[test]
fn percentile_vectors() {
    let v: Vec<f64> = (1..=11).map(f64::from).collect();
    assert_eq!(percentile(&v, 0.10), 2.0);
    assert_eq!(percentile(&v, 0.90), 10.0);
    assert!((percentile(&[1.0, 2.0, 3.0, 4.0], 0.9) - 3.7).abs() < 1e-12);
    assert_eq!(percentile(&[], 0.5), 0.0);
    assert_eq!(percentile(&[7.0], 0.9), 7.0);
}

#[test]
fn jsonl_vault_lines_are_aggregated() {
    let text = [
        json!({"schemaVersion": "1.1.0", "sessionId": "A"}).to_string(),
        score_record(
            24,
            vec![(
                "stim_05_geopref_split_10s",
                vec![
                    row("social_dwell_ratio", 0.72, false),
                    row("social_dwell_ratio", 0.68, false),
                ],
            )],
        )
        .to_string(),
    ]
    .join("\n");
    let buckets = extract(&load_session_records(&text));
    let k = key("stim_05_geopref_split_10s", "social_dwell_ratio", "24-35");
    assert_eq!(buckets[&k].len(), 2);
}

#[test]
fn imputed_rows_are_excluded() {
    let rec = score_record(
        20,
        vec![(
            "stim_02_gaze_cue_right_8s",
            vec![
                row("first_entry_latency_ms", 8000.0, true),
                row("first_entry_latency_ms", 540.0, false),
            ],
        )],
    );
    let buckets = extract(std::slice::from_ref(&rec));
    let k = key(
        "stim_02_gaze_cue_right_8s",
        "first_entry_latency_ms",
        "18-23",
    );
    assert_eq!(buckets[&k], vec![540.0]);

    // include_imputed opts the sentinel back in.
    let mut with = Buckets::new();
    let p = AggregateParams {
        include_imputed: true,
        ..Default::default()
    };
    extract_observations(&[rec], &p, &mut with);
    assert_eq!(with[&k], vec![8000.0, 540.0]);
}

#[test]
fn researchkit_share_export_shape() {
    let rec = score_record(
        40,
        vec![(
            "stim_01_social_peekaboo_8s",
            vec![row("eye_dwell_ratio", 0.33, false)],
        )],
    );
    let share = json!({"per_video": rec["score"]}).to_string();
    let buckets = extract(&load_session_records(&share));
    assert_eq!(
        buckets[&key("stim_01_social_peekaboo_8s", "eye_dwell_ratio", "36-47")],
        vec![0.33]
    );

    // Divergence from Python: a pretty-printed single object is also accepted.
    let pretty = serde_json::to_string_pretty(&json!({"per_video": rec["score"]})).unwrap();
    assert_eq!(load_session_records(&pretty).len(), 1);
}

#[test]
fn out_of_range_age_is_skipped() {
    let rec = score_record(
        9,
        vec![(
            "stim_05_geopref_split_10s",
            vec![row("social_dwell_ratio", 0.5, false)],
        )],
    );
    assert!(extract(&[rec]).is_empty());
}

fn seed_social(n: usize, age: i64) -> Vec<Value> {
    (0..n)
        .map(|i| {
            score_record(
                age,
                vec![(
                    "stim_05_geopref_split_10s",
                    vec![row(
                        "social_dwell_ratio",
                        0.60 + (i % 5) as f64 * 0.02,
                        false,
                    )],
                )],
            )
        })
        .collect()
}

#[test]
fn cells_below_min_n_are_withheld() {
    let set = aggregate(&seed_social(5, 24), &AggregateParams::default());
    let e = &set.videos["stim_05_geopref_split_10s"].segments[0].expectations[0];
    assert!(e.by_age.values().all(Option::is_none));
    assert_eq!(e.by_age.len(), 5);
    assert_eq!(e.source, "collected-candidate");
    assert_eq!(qualifying_cells(&set), 0);
}

#[test]
fn qualifying_cells_emit_median_and_p10() {
    let set = aggregate(&seed_social(12, 24), &AggregateParams::default());
    let e = &set.videos["stim_05_geopref_split_10s"].segments[0].expectations[0];
    let cell = e.by_age["24-35"].as_ref().expect("cell");
    assert_eq!(cell.n, 12);
    assert!((cell.expected - 0.63).abs() < 1e-6);
    assert!(cell.acceptable_low.is_some() && cell.acceptable_high.is_none());
    assert_eq!(e.direction, NormativeDirection::HigherBetter);
    assert_eq!(set.status, "candidate-collected");
    assert_eq!(set.schema_version, "1.0.0-candidate");
}

#[test]
fn latency_direction_is_lower_better() {
    let recs: Vec<Value> = (0..12)
        .map(|i| {
            score_record(
                28,
                vec![(
                    "stim_01_social_peekaboo_8s",
                    vec![row(
                        "first_entry_latency_ms",
                        300.0 + i as f64 * 10.0,
                        false,
                    )],
                )],
            )
        })
        .collect();
    let set = aggregate(&recs, &AggregateParams::default());
    let e = &set.videos["stim_01_social_peekaboo_8s"].segments[0].expectations[0];
    assert_eq!(e.direction, NormativeDirection::LowerBetter);
    let cell = e.by_age["24-35"].as_ref().unwrap();
    assert!(cell.acceptable_high.is_some() && cell.acceptable_low.is_none());
}

#[test]
fn zero_spread_cells_are_withheld() {
    let recs: Vec<Value> = (0..20)
        .map(|_| score_record(24, vec![("s", vec![row("social_dwell_ratio", 0.5, false)])]))
        .collect();
    let set = aggregate(&recs, &AggregateParams::default());
    assert_eq!(qualifying_cells(&set), 0);
}

#[test]
fn bad_jsonl_lines_are_skipped() {
    let text = format!(
        "not json\n\n{}\n",
        score_record(24, vec![("s", vec![row("b", 1.0, false)])])
    );
    assert_eq!(load_session_records(&text).len(), 1);
    assert!(load_session_records("   ").is_empty());
}

#[test]
fn candidate_document_matches_golden() {
    // Two stimuli, two brackets, mixed directions, one withheld cell.
    let mut recs = seed_social(14, 24);
    for i in 0..13 {
        recs.push(score_record(
            30,
            vec![(
                "stim_01_social_peekaboo_8s",
                vec![
                    row("first_entry_latency_ms", 300.0 + i as f64 * 17.0, false),
                    row("eye_dwell_ratio", 0.2 + (i % 4) as f64 * 0.05, false),
                ],
            )],
        ));
    }
    recs.extend(seed_social(3, 14));
    let set = aggregate(&recs, &AggregateParams::default());
    let actual = serde_json::to_string_pretty(&set).unwrap() + "\n";
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/golden/norms/candidate.json"
    );
    if std::env::var("ASD_WRITE_GOLDEN").as_deref() == Ok("1") {
        std::fs::write(path, &actual).unwrap();
    }
    let expected =
        std::fs::read_to_string(path).expect("golden file (run with ASD_WRITE_GOLDEN=1)");
    assert_eq!(actual, expected);
}

#[test]
fn build_candidate_baseline_from_empty_buckets() {
    let set = build_candidate_baseline(&Buckets::new(), &AggregateParams::default());
    assert!(set.videos.is_empty());
    assert_eq!(set.age_brackets.len(), 5);
}
