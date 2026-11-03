//! owner: WP-X.9
//!
//! Phase-B analysis runs (SWP-1..3, AGE-1, AGE-3, PRV-1, RUN-4) over the `analysis_support`
//! cohort (30 research-consented sessions, 10 with a research frame log; bands 12-17: 5,
//! 18-23: 5, 24-35: 10, 36-47: 10):
//!
//! - sweep refusals before anything is queued: registry shape, level, capture-only, integer
//!   and step rules, 25 steps, the evaluation budget counting the published cell, the summary
//!   estimate; an owner-gated (`H-G2`) knob is accepted (ADR-0012 Q9);
//! - a score-level `param_sweep` end to end: the published value is a cell, per-cell rates
//!   and the RUN-4 blocks of the nearest cell agree, X.8's loader reads the run;
//! - a frames-level sweep of the I-VT threshold moves the fixation count;
//! - an `age_profile` run against a draft baselines version moves the shares, and bands under
//!   `max(k, norms.minN)` are suppressed.
//!
//! Needs `DATABASE_URL`; skipped otherwise.

mod analysis_support;
mod common;
mod research_support;

use analysis_support::*;
use asd_backend::config_docs::seed::NORMATIVE_BASELINES_JSON;
use asd_contract::api_v1::Role;
use http::StatusCode;
use research_support::*;
use serde_json::{json, Value};
use uuid::Uuid;

fn slug(body: &Value) -> &str {
    common::auth::slug(body)
}

fn sweep_body(experiment: &str, level: &str, axes: Value) -> Value {
    json!({
        "kind": "param_sweep",
        "experiment_id": experiment,
        "cohort": {},
        "level": level,
        "seed": 42,
        "sweep": {"axes": axes},
    })
}

fn axis(id: &str, from: f64, to: f64, step: f64) -> Value {
    json!({"param_id": id, "from": from, "to": to, "step": step})
}

async fn set_small_cell_k(c: &Ctx, k: u32) {
    sqlx::query(
        "UPDATE organizations
         SET settings = jsonb_set(COALESCE(settings, '{}'::jsonb), '{research}',
                                  jsonb_build_object('small_cell_k', $2::int))
         WHERE id = $1",
    )
    .bind(c.org)
    .bind(k as i32)
    .execute(&c.db.pool)
    .await
    .unwrap();
}

async fn run_to_end(c: &Ctx, body: Value, bearer: &str) -> Value {
    let (s, run) = create_run(c, body, bearer).await;
    assert_eq!(s, StatusCode::ACCEPTED, "{run}");
    assert!(c.run_jobs().await >= 1);
    let run = get_run(c, run["id"].as_str().unwrap(), bearer).await;
    assert_eq!(run["status"], "succeeded", "{run}");
    run
}

#[tokio::test]
async fn sweep_refusals_happen_before_queueing() {
    let Some(c) = ctx_with(|cfg| cfg.analysis.max_evaluations = 120).await else {
        return;
    };
    cohort(&c, None).await;
    let (_, researcher) = c.token(Role::Researcher).await;
    let (exp, _) = experiment(&c, &researcher).await;
    let refused = |b: &Value| {
        b["invalid_params"][0]["name"]
            .as_str()
            .unwrap_or("")
            .to_owned()
    };

    let cases = [
        // (level, axes, expected invalid param name)
        (
            "score",
            json!([axis("no.such.knob", 0.0, 1.0, 0.1)]),
            "sweep.axes[0].param_id",
        ),
        (
            "score",
            json!([axis("metrics.ivtUnit", 0.0, 1.0, 1.0)]),
            "sweep.axes[0].param_id",
        ),
        // A frames knob at the score level; a capture-only knob at any level.
        (
            "score",
            json!([axis("metrics.ivtVelocityUnitsPerSec", 1.0, 5.0, 1.0)]),
            "sweep.axes[0].param_id",
        ),
        (
            "frames",
            json!([axis(
                "trackingQuality.calibration.maxRetriesPerTarget",
                0.0,
                5.0,
                1.0
            )]),
            "sweep.axes[0].param_id",
        ),
        (
            "score",
            json!([axis("norms.minN", 5.5, 10.0, 1.0)]),
            "sweep.axes[0]",
        ),
        (
            "score",
            json!([axis("flag.greenFloor", 0.5, 0.6, 0.001)]),
            "sweep.axes[0].step",
        ),
        (
            "score",
            json!([axis("flag.greenFloor", 0.5, 1.5, 0.1)]),
            "sweep.axes[0]",
        ),
        (
            "score",
            json!([axis("flag.greenFloor", 0.6, 0.5, 0.1)]),
            "sweep.axes[0]",
        ),
        (
            "score",
            json!([
                axis("flag.greenFloor", 0.5, 0.6, 0.1),
                axis("flag.greenFloor", 0.5, 0.6, 0.1)
            ]),
            "sweep.axes",
        ),
        ("score", json!([]), "sweep.axes"),
    ];
    for (level, axes, name) in cases {
        let (s, b) = create_run(&c, sweep_body(&exp, level, axes.clone()), &researcher).await;
        assert_eq!(s, StatusCode::BAD_REQUEST, "{axes}: {b}");
        assert_eq!(refused(&b), name, "{axes}: {b}");
    }
    // 25 steps on one axis (at most 24).
    let (s, b) = create_run(
        &c,
        sweep_body(
            &exp,
            "score",
            json!([axis("flag.greenFloor", 0.5, 0.75, 0.01)]),
        ),
        &researcher,
    )
    .await;
    assert_eq!(
        (s, slug(&b)),
        (StatusCode::BAD_REQUEST, "sweep-too-large"),
        "{b}"
    );
    // Budget 120 evaluations, 30 score sessions: 4 grid cells fit (120), but the published
    // 0.65 is off the 0.5..0.8 step 0.1 grid and is a fifth cell (150).
    let (s, b) = create_run(
        &c,
        sweep_body(
            &exp,
            "score",
            json!([axis("flag.greenFloor", 0.5, 0.8, 0.1)]),
        ),
        &researcher,
    )
    .await;
    assert_eq!(
        (s, slug(&b)),
        (StatusCode::BAD_REQUEST, "sweep-too-large"),
        "{b}"
    );
    assert!(b["detail"].as_str().unwrap().contains("150"), "{b}");
    // Every metric in 625 cells would not fit the 1 MiB summary.
    let mut big = sweep_body(
        &exp,
        "score",
        json!([
            axis("flag.greenFloor", 0.0, 0.96, 0.04),
            axis("flag.yellowFloor", 0.0, 0.96, 0.04)
        ]),
    );
    big["options"] = json!({"metrics": asd_core::metrics::PerVideoMetricKey::ALL
        .iter()
        .map(|k| k.as_str())
        .collect::<Vec<_>>()});
    let (s, b) = create_run(&c, big, &researcher).await;
    assert_eq!(
        (s, slug(&b)),
        (StatusCode::BAD_REQUEST, "sweep-too-large"),
        "{b}"
    );
    assert!(
        b["detail"]
            .as_str()
            .unwrap()
            .contains("ASD_ANALYSIS_MAX_SUMMARY_KIB"),
        "{b}"
    );
    assert_eq!(c.audit_rows("analysis.run_enqueue").await.len(), 0);

    // An owner-gated knob (H-G2) sweeps in analysis (ADR-0012 Q9): 3 cells + published = 120.
    let (s, run) = create_run(
        &c,
        sweep_body(
            &exp,
            "score",
            json!([axis(
                "trackingQuality.scoring.maxAccuracyDegForScoring",
                1.0,
                3.0,
                1.0
            )]),
        ),
        &researcher,
    )
    .await;
    assert_eq!(s, StatusCode::ACCEPTED, "{run}");
    assert_eq!(run["kind"], "param_sweep");
    assert_eq!(c.audit_rows("analysis.run_enqueue").await.len(), 1);
    c.db.cleanup().await;
}

#[tokio::test]
async fn score_sweep_marks_the_published_cell_and_agrees_with_the_run_blocks() {
    let Some(c) = ctx_with(|_| {}).await else {
        return;
    };
    cohort(&c, None).await;
    let (_, researcher) = c.token(Role::Researcher).await;
    let (exp, _) = experiment(&c, &researcher).await;
    let mut body = sweep_body(
        &exp,
        "score",
        json!([axis("flag.greenFloor", 0.0, 1.0, 0.25)]),
    );
    body["options"] = json!({"metrics": ["eye_dwell_ratio"]});
    let run = run_to_end(&c, body, &researcher).await;
    let summary = &run["summary"];
    let sweep = &summary["sweep"];
    assert_eq!(sweep["axes"][0]["param_id"], "flag.greenFloor");
    let published = sweep["published_values"][0].as_f64().unwrap();
    let cells = sweep["cells"].as_array().unwrap();
    // 0, .25, .5, .75, 1 and the published value (off the grid) last.
    assert_eq!(cells.len(), 6, "{sweep}");
    assert_eq!(cells[5]["values"], json!([published]));
    let mut greens = Vec::new();
    for cell in cells {
        assert_eq!(cell["n"], 30, "{cell}");
        let r = &cell["flag_rate"];
        let total: f64 = ["green", "yellow", "red", "unreliable"]
            .iter()
            .map(|f| r[f].as_f64().unwrap())
            .sum();
        assert!((total - 1.0).abs() < 1e-9);
        assert!(cell["metrics"]["eye_dwell_ratio"]["mean"]
            .as_f64()
            .is_some());
        assert!(
            cell["metrics"].get("fixation_count").is_none(),
            "chosen metrics only"
        );
        // The green floor never moves the composite.
        assert_eq!(cell["composite"], cells[0]["composite"]);
        greens.push(r["green"].as_f64().unwrap());
    }
    assert!(greens[..5].windows(2).all(|w| w[0] >= w[1]), "{greens:?}");
    // The rows and the RUN-4 blocks are the published profile vs the cell nearest the
    // experiment's own 0.65: 0.75 (cell 3).
    assert_eq!(summary["overall"]["n"], 30);
    assert_eq!(summary["counts"]["by_level"]["score"], 30);
    let near = &cells[3]["flag_rate"];
    for f in ["green", "yellow", "red", "unreliable"] {
        if let Some(n) = summary["flags"]["draft"][f].as_u64() {
            let expected = (near[f].as_f64().unwrap() * 30.0).round() as u64;
            assert_eq!(n, expected, "{f}: {summary}");
        }
    }
    assert_eq!(
        summary["counts"]["params_not_applied"]["metrics.ivtVelocityUnitsPerSec"], 30,
        "{summary}"
    );
    // RUN-4 cap held; X.8 reads the run and its rows.
    assert!(serde_json::to_vec(summary).unwrap().len() < 1024 * 1024);
    let id: Uuid = run["id"].as_str().unwrap().parse().unwrap();
    let record = asd_backend::export_build::analysis::load_run(&c.db.pool, c.org, id)
        .await
        .unwrap();
    assert_eq!(record.summary.sweep.as_ref().unwrap().cells.len(), 6);
    let rows = asd_backend::export_build::analysis::load_rows(
        c.state.blobs.as_ref(),
        c.state.keyring.as_deref().unwrap(),
        c.org,
        id,
        record.blob_key.as_deref().unwrap(),
    )
    .await
    .unwrap();
    assert_eq!(rows.len(), 30, "one row per session, not per cell");
    c.db.cleanup().await;
}

#[tokio::test]
async fn frames_sweep_moves_the_fixation_count() {
    let Some(c) = ctx_with(|_| {}).await else {
        return;
    };
    cohort(&c, None).await;
    set_small_cell_k(&c, 5).await;
    let (_, researcher) = c.token(Role::Researcher).await;
    let (exp, _) = experiment(&c, &researcher).await;
    let mut body = sweep_body(
        &exp,
        "frames",
        json!([axis("metrics.ivtVelocityUnitsPerSec", 1.0, 9.0, 4.0)]),
    );
    body["options"] = json!({"metrics": ["fixation_count"]});
    let run = run_to_end(&c, body, &researcher).await;
    let summary = &run["summary"];
    assert_eq!(summary["counts"]["by_level"]["frames"], 10);
    let cells = summary["sweep"]["cells"].as_array().unwrap();
    // 1, 5 (the published value, on the grid), 9.
    assert_eq!(cells.len(), 3, "{cells:?}");
    assert_eq!(summary["sweep"]["published_values"], json!([5.0]));
    let fixations: Vec<f64> = cells
        .iter()
        .map(|c| c["metrics"]["fixation_count"]["mean"].as_f64().unwrap())
        .collect();
    assert!(
        fixations.windows(2).any(|w| w[0] != w[1]),
        "the I-VT threshold moves the fixation count: {fixations:?}"
    );
    assert!(summary["counts"]["params_not_applied"]
        .get("metrics.ivtVelocityUnitsPerSec")
        .is_none());
    c.db.cleanup().await;
}

#[tokio::test]
async fn age_profile_with_a_draft_baseline_moves_the_shares_and_suppresses_small_bands() {
    let Some(c) = ctx_with(|_| {}).await else {
        return;
    };
    cohort(&c, None).await;
    set_small_cell_k(&c, 5).await;
    let (_, researcher) = c.token(Role::Researcher).await;
    let (exp, _) = experiment(&c, &researcher).await;
    // A draft baselines version whose higherBetter ranges no session reaches.
    let mut draft: Value = serde_json::from_str(NORMATIVE_BASELINES_JSON).unwrap();
    for video in draft["videos"].as_object_mut().unwrap().values_mut() {
        for seg in video["segments"].as_array_mut().unwrap() {
            for e in seg["expectations"].as_array_mut().unwrap() {
                if e["direction"] == "higherBetter" {
                    for range in e["byAge"].as_object_mut().unwrap().values_mut() {
                        range["expected"] = json!(1e6);
                        range["acceptableLow"] = json!(1e5);
                        range.as_object_mut().unwrap().remove("acceptableHigh");
                    }
                }
            }
        }
    }
    sqlx::query(
        "INSERT INTO config_documents (kind, version, status, body)
         VALUES ('normative_baselines', 'draft-age-test', 'draft', $1)",
    )
    .bind(&draft)
    .execute(&c.db.pool)
    .await
    .unwrap();
    // The experiment names the draft baselines and norms.minN 10 (AGE-3: max(k 5, 10)).
    sqlx::query(
        "UPDATE experiments
         SET baselines_version = 'draft-age-test',
             profile = jsonb_set(profile, '{norms,minN}', '10'::jsonb)
         WHERE id = $1::uuid",
    )
    .bind(&exp)
    .execute(&c.db.pool)
    .await
    .unwrap();
    let body = json!({
        "kind": "age_profile",
        "experiment_id": exp,
        "cohort": {},
        "level": "score",
        "seed": 42,
    });
    let run = run_to_end(&c, body, &researcher).await;
    assert_eq!(run["baselines_version"], "draft-age-test");
    let age = &run["summary"]["age_profile"];
    assert_eq!(age["baselines"]["draft"], "draft-age-test", "{age}");
    assert_ne!(age["baselines"]["published"], "draft-age-test");
    assert_eq!(age["min_n"], 10);
    assert_eq!(age["suppression_threshold"], 10);
    let bands = age["bands"].as_array().unwrap();
    let by_id = |id: &str| bands.iter().find(|b| b["band"] == id).unwrap();
    // 5 sessions each: suppressed although k is 5.
    for id in ["12-17", "18-23"] {
        let b = by_id(id);
        assert_eq!(b["suppressed"], true, "{b}");
        assert_eq!(b["n"], Value::Null);
        for e in b["expectations"].as_array().unwrap() {
            assert_eq!(e["published"]["shares"], Value::Null, "{e}");
            assert_eq!(e["published"]["dist"]["mean"], Value::Null, "{e}");
        }
    }
    for id in ["24-35", "36-47"] {
        assert_eq!(by_id(id)["n"], 10);
    }
    // The RUN-4 blocks are there too (an age profile is a re-score plus the block).
    assert_eq!(run["summary"]["overall"]["n"], 30);

    // norms.minN 5: the per-expectation cells (fewer sessions than the band: not every
    // session saw every clip) show their shares; the draft range makes every higherBetter
    // cell atypical while the published shares stay a distribution.
    sqlx::query(
        "UPDATE experiments SET profile = jsonb_set(profile, '{norms,minN}', '5'::jsonb)
         WHERE id = $1::uuid",
    )
    .bind(&exp)
    .execute(&c.db.pool)
    .await
    .unwrap();
    let body = json!({
        "kind": "age_profile",
        "experiment_id": exp,
        "cohort": {},
        "level": "score",
        "seed": 42,
    });
    let run = run_to_end(&c, body, &researcher).await;
    let age = &run["summary"]["age_profile"];
    assert_eq!(age["suppression_threshold"], 5);
    let mut checked = 0;
    for b in age["bands"].as_array().unwrap() {
        for e in b["expectations"].as_array().unwrap() {
            if e["published"]["suppressed"] == true {
                assert_eq!(e["published"]["shares"], Value::Null, "{e}");
                continue;
            }
            assert!(e["published"]["n"].as_u64().unwrap() >= 5, "{e}");
            let p = &e["published"]["shares"];
            let sum: f64 = ["on_target", "monitor", "atypical"]
                .iter()
                .map(|k| p[k].as_f64().unwrap())
                .sum();
            assert!((sum - 1.0).abs() < 1e-9, "{e}");
            if e["direction"] == "higherBetter" {
                assert_eq!(e["draft"]["shares"]["atypical"], 1.0, "{e}");
                assert_eq!(e["draft"]["expected_percentile"], 100.0, "{e}");
                assert!(p["atypical"].as_f64().unwrap() < 1.0, "{e}");
                checked += 1;
            }
        }
    }
    assert!(checked > 0, "{age}");
    // The RUN-4 blocks are there too (an age profile is a re-score plus the block).
    assert_eq!(run["summary"]["overall"]["n"], 30);
    c.db.cleanup().await;
}
