//! owner: WP-X.0
//!
//! Researcher toolkit DTOs (EXP, COH, RUN, GOV-6): key names round-trip, every type is
//! published in `api-v1.json`, real instances validate against their definitions, the new
//! problem slugs are listed, and the export contract gains `analysis_results` (RUN-7).

use std::collections::BTreeMap;

use asd_contract::api_v1::{ConfigDocumentKind, JobProgress, ResearchFramesState};
use asd_contract::config::ScoringProfile;
use asd_contract::export::{
    AnalysisEncoding, AnalysisExport, AnalysisLayout, ExportFormat, ExportMode, ExportPurpose,
    ExportRequest,
};
use asd_contract::problem;
use asd_contract::research::*;
use asd_contract::schema;
use asd_core::types::tracking::TrackingVerdict;
use asd_core::types::GazeQualityVerdict;
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::{json, Value};

fn round_trip<T: Serialize + DeserializeOwned + PartialEq + std::fmt::Debug>(v: &T) -> Value {
    let json = serde_json::to_value(v).unwrap();
    let back: T = serde_json::from_value(json.clone()).unwrap();
    assert_eq!(&back, v);
    json
}

/// Validates `instance` against `$defs/<name>` of api-v1.json, unknown keys forbidden.
fn validates_as(name: &str, instance: &Value) {
    let mut api = schema::api_v1();
    schema::forbid_additional_properties(&mut api);
    api["$ref"] = json!(format!("#/$defs/{name}"));
    schema::validate(&api, instance).unwrap_or_else(|e| panic!("{name}: {e:?}"));
}

fn cohort() -> CohortDefinition {
    CohortDefinition {
        age_bands: vec!["24-35".into(), "36-47".into()],
        sex: vec![],
        device_ids: vec!["9a8b7c6d-5e4f-4321-8fed-cba987654321".into()],
        from: Some("2026-01-01".into()),
        to: None,
        quality_verdicts: vec![GazeQualityVerdict::Optimal, GazeQualityVerdict::Acceptable],
        tracking_verdicts: vec![TrackingVerdict::Good, TrackingVerdict::Fair],
        min_valid_ratio: Some(0.7),
        scoring_status: vec![],
        require_frames: false,
        include_synthetic: false,
    }
}

fn dist(n: u32) -> Dist {
    Dist {
        n: Some(n),
        suppressed: false,
        mean: Some(0.5),
        sd: Some(0.1),
        median: Some(0.5),
        iqr: Some([0.4, 0.6]),
        p10: Some(0.35),
        p90: Some(0.65),
        ci95_mean: Some([0.45, 0.55]),
        hist: Some(Hist {
            edges: vec![0.0, 0.5, 1.0],
            counts: vec![n / 2, n - n / 2],
        }),
    }
}

fn suppressed() -> Dist {
    Dist {
        suppressed: true,
        ..Dist::default()
    }
}

fn summary() -> RunSummary {
    let band = BandSummary {
        band: "24-35".into(),
        n: Some(30),
        suppressed: false,
        metrics: BTreeMap::from([(
            "eye_dwell_ratio".to_string(),
            DistComparison {
                published: dist(30),
                draft: dist(30),
                delta: dist(30),
            },
        )]),
        composite: DistComparison {
            published: dist(30),
            draft: dist(30),
            delta: dist(30),
        },
        flag_rate: FlagRateComparison {
            published: Some(FlagRates {
                green: 0.5,
                yellow: 0.3,
                red: 0.1,
                unreliable: 0.1,
            }),
            draft: None,
        },
    };
    let small = BandSummary {
        band: "36-47".into(),
        n: None,
        suppressed: true,
        metrics: BTreeMap::new(),
        composite: DistComparison {
            published: suppressed(),
            draft: suppressed(),
            delta: suppressed(),
        },
        flag_rate: FlagRateComparison::default(),
    };
    RunSummary {
        counts: RunCounts {
            sessions: 37,
            by_level: LevelCounts {
                frames: 10,
                score: 27,
            },
            excluded: BTreeMap::from([("no_age".to_string(), 2)]),
            params_not_applied: BTreeMap::from([(
                "metrics.ivtVelocityUnitsPerSec".to_string(),
                27,
            )]),
            suppressed_cells: 1,
        },
        flags: FlagSummary {
            published: FlagCounts {
                green: Some(20),
                yellow: Some(12),
                red: None,
                unreliable: Some(0),
            },
            draft: FlagCounts::default(),
            transitions: vec![
                ("green".into(), "yellow".into(), Some(13)),
                ("yellow".into(), "red".into(), None),
            ],
        },
        quality: QualitySummary::default(),
        by_age_band: vec![band.clone(), small],
        overall: BandSummary {
            band: "overall".into(),
            ..band
        },
        sweep: None,
        age_profile: None,
        small_cell_k: DEFAULT_SMALL_CELL_K,
        includes_synthetic: false,
    }
}

fn run() -> AnalysisRun {
    AnalysisRun {
        id: "11111111-1111-4111-8111-111111111111".into(),
        org_id: "5f0e1d2c-3b4a-4968-8776-655443322110".into(),
        kind: RunKind::CohortRescore,
        status: RunStatus::Succeeded,
        experiment_id: Some("22222222-2222-4222-8222-222222222222".into()),
        cohort_id: None,
        definition: AnalysisDefinition {
            kind: RunKind::CohortRescore,
            cohort: cohort(),
            profile: ScoringProfile::default(),
            baselines_version: None,
            sweep: None,
            level: LevelRequest::Auto,
            options: RunOptions::default(),
            seed: 42,
        },
        definition_hash: "ab".repeat(32),
        cohort_hash: "cd".repeat(32),
        membership_hash: Some("ef".repeat(32)),
        profile_hash: "01".repeat(32),
        published_profile_version: "1.1.0".into(),
        baselines_version: "2026.10.0".into(),
        catalog_version: "1.4.0".into(),
        bundle_version: None,
        core_version: "0.1.0".into(),
        contract_version: "1".into(),
        pipeline_version: "1".into(),
        registry_version: "1.0.0".into(),
        seed: 42,
        level_requested: LevelRequest::Auto,
        sessions_n: Some(37),
        sessions_frames_n: Some(10),
        sessions_score_n: Some(27),
        excluded: Some(BTreeMap::from([("no_age".to_string(), 2)])),
        summary: Some(summary()),
        result_bytes: Some(4096),
        progress: Some(JobProgress {
            done: 37,
            total: 37,
            phase: "done".into(),
        }),
        job_id: Some("33333333-3333-4333-8333-333333333333".into()),
        error: None,
        membership_drift: None,
        created_by: None,
        created_at: "2026-11-03T10:00:00Z".into(),
        started_at: Some("2026-11-03T10:00:01Z".into()),
        finished_at: Some("2026-11-03T10:01:00Z".into()),
    }
}

#[test]
fn analysis_run_round_trips_and_validates() {
    let v = round_trip(&run());
    assert_eq!(v["definition"]["level"], "auto");
    assert_eq!(
        v["summary"]["flags"]["transitions"][1],
        json!(["yellow", "red", null])
    );
    assert_eq!(
        v["summary"]["by_age_band"][1]["composite"]["draft"]["n"],
        Value::Null
    );
    assert_eq!(v["summary"]["by_age_band"][1]["suppressed"], true);
    validates_as("AnalysisRun", &v);
    validates_as(
        "AnalysisRunPage",
        &json!({"items": [v], "next_cursor": null}),
    );
}

#[test]
fn run_request_defaults_and_validation() {
    let req: AnalysisRunRequest = serde_json::from_value(json!({
        "kind": "cohort_rescore",
        "experiment_id": "22222222-2222-4222-8222-222222222222",
        "cohort_id": "44444444-4444-4444-8444-444444444444"
    }))
    .unwrap();
    assert_eq!(req.level, LevelRequest::Auto);
    assert!(req.options.bootstrap, "bootstrap defaults to true");
    assert!(req.options.metrics.is_empty());
    validates_as("AnalysisRunRequest", &round_trip(&req));

    let sweep = AnalysisRunRequest {
        kind: RunKind::ParamSweep,
        cohort: Some(cohort()),
        cohort_id: None,
        sweep: Some(SweepSpec {
            axes: vec![SweepAxis {
                param_id: "flag.greenFloor".into(),
                from: 0.5,
                to: 0.8,
                step: 0.05,
            }],
        }),
        level: LevelRequest::Score,
        seed: Some(7),
        ..req
    };
    validates_as("AnalysisRunRequest", &round_trip(&sweep));
    assert!(serde_json::from_value::<AnalysisRunRequest>(json!({
        "kind": "cohort_rescore", "experiment_id": "x", "bogus": 1
    }))
    .is_err());
}

#[test]
fn experiments_and_cohorts_round_trip() {
    let e = Experiment {
        id: "22222222-2222-4222-8222-222222222222".into(),
        org_id: "5f0e1d2c-3b4a-4968-8776-655443322110".into(),
        name: "lower I-VT".into(),
        description: None,
        status: ExperimentStatus::Draft,
        base_profile_version: "1.1.0".into(),
        profile: ScoringProfile::default(),
        profile_hash: "01".repeat(32),
        baselines_version: None,
        promoted_config_version: None,
        last_run_id: None,
        row_version: 1,
        created_by: Some("55555555-5555-4555-8555-555555555555".into()),
        created_at: "2026-11-03T10:00:00Z".into(),
        updated_at: "2026-11-03T10:00:00Z".into(),
    };
    let v = round_trip(&e);
    assert_eq!(v["status"], "draft");
    validates_as("Experiment", &v);

    let patch: UpdateExperimentRequest = serde_json::from_value(
        json!({"profile": serde_json::to_value(ScoringProfile::default()).unwrap()}),
    )
    .unwrap();
    assert!(patch.profile.is_some() && patch.name.is_none());
    let promote = PromoteExperimentRequest {
        kind: ConfigDocumentKind::ScoringProfile,
        version: "1.2.0-exp1".into(),
        notes: Some("cohort A".into()),
    };
    validates_as("PromoteExperimentRequest", &round_trip(&promote));
    validates_as(
        "CreateExperimentRequest",
        &round_trip(&CreateExperimentRequest {
            name: "x".into(),
            description: None,
            from_experiment_id: None,
            from_version: Some("1.1.0".into()),
        }),
    );

    let c = Cohort {
        id: "44444444-4444-4444-8444-444444444444".into(),
        org_id: "5f0e1d2c-3b4a-4968-8776-655443322110".into(),
        name: "24-47 months".into(),
        definition: cohort(),
        definition_hash: "cd".repeat(32),
        created_by: None,
        created_at: "2026-11-03T10:00:00Z".into(),
        updated_at: "2026-11-03T10:00:00Z".into(),
        archived_at: None,
    };
    let v = round_trip(&c);
    assert_eq!(
        v["definition"]["quality_verdicts"],
        json!(["optimal", "acceptable"])
    );
    assert_eq!(
        v["definition"]["tracking_verdicts"],
        json!(["good", "fair"])
    );
    validates_as("Cohort", &v);
    // An empty definition is every consented session.
    let empty: CohortDefinition = serde_json::from_value(json!({})).unwrap();
    assert_eq!(empty, CohortDefinition::default());

    let preview = CohortPreview {
        n: Some(37),
        by_age_band: vec![
            CohortBandCount {
                band: "24-35".into(),
                n: Some(30),
                frames_n: Some(10),
                suppressed: false,
            },
            CohortBandCount {
                band: "36-47".into(),
                n: None,
                frames_n: None,
                suppressed: true,
            },
        ],
        by_level: LevelCounts {
            frames: 10,
            score: 27,
        },
        excluded: CohortExclusions::default(),
        small_cell_k: 12,
    };
    validates_as("CohortPreview", &round_trip(&preview));
}

#[test]
fn session_rows_and_effective_config_round_trip() {
    let row = AnalysisSessionRow {
        session_id: "SESS-1".into(),
        level: Level::Frames,
        age_months: Some(30),
        age_band: Some("24-35".into()),
        device_id: None,
        published: SessionSide {
            composite: Some(0.6),
            flag: Some(asd_contract::api_v1::SessionAttentionFlag::Yellow),
            quality_status: Some("ok".into()),
            metrics: BTreeMap::from([("eye_dwell_ratio".to_string(), Some(0.33))]),
        },
        draft: SessionSide::default(),
        delta: SessionDelta::default(),
        warnings: vec!["param_not_applied: aoi.marginNorm".into()],
    };
    let v = round_trip(&row);
    validates_as(
        "AnalysisSessionPage",
        &json!({"items": [v], "next_cursor": "c1"}),
    );
    let eff = EffectiveConfig {
        source: EffectiveSource::Global,
        bundle_version: None,
        scoring_profile: EffectiveDocument {
            version: "1.1.0".into(),
            body: serde_json::to_value(ScoringProfile::default()).unwrap(),
        },
        normative_baselines: EffectiveDocument {
            version: "b1".into(),
            body: json!({}),
        },
        stimulus_catalog: EffectiveDocument {
            version: "c1".into(),
            body: json!({}),
        },
    };
    assert_eq!(round_trip(&eff)["source"], "global");
    let frames = ResearchFramesState {
        available: true,
        purged: false,
        bytes: Some(3_000_000),
    };
    validates_as("ResearchFramesState", &round_trip(&frames));
}

#[test]
fn every_toolkit_slug_is_published() {
    for slug in [
        problem::EXPERIMENT_NAME_EXISTS,
        problem::EXPERIMENT_PROMOTED,
        problem::EXPERIMENT_NO_RUN,
        problem::COHORT_EMPTY,
        problem::COHORT_TOO_LARGE,
        problem::SWEEP_TOO_LARGE,
        problem::ANALYSIS_QUOTA_EXCEEDED,
        problem::ANALYSIS_LEVEL_UNAVAILABLE,
        problem::FRAMES_NOT_AVAILABLE,
        problem::FRAMES_PURGED,
        problem::RESEARCH_CONSENT_REQUIRED,
        problem::DIGEST_REQUIRED,
        problem::CONFIG_UNANALYSED,
        problem::JOB_FINISHED,
        problem::RESULT_TOO_LARGE,
        problem::NOT_IMPLEMENTED,
    ] {
        assert!(problem::SLUGS.contains(&slug), "{slug}");
    }
    let api = schema::api_v1();
    let published = api["$defs"]["ProblemSlug"]["enum"].as_array().unwrap();
    assert!(published.contains(&json!("experiment-no-run")));
    assert!(published.contains(&json!("not-implemented")));
}

#[test]
fn analysis_results_export_contract() {
    assert_eq!(ExportFormat::AnalysisResults.as_str(), "analysis_results");
    assert_eq!(
        ExportFormat::parse("analysis_results"),
        Some(ExportFormat::AnalysisResults)
    );
    let req = ExportRequest {
        format: ExportFormat::AnalysisResults,
        mode: ExportMode::Deidentified,
        purpose: ExportPurpose::Research,
        filters: Default::default(),
        destination_ids: vec![],
        reason: "cohort A analysis for the paper".into(),
        include_labels: false,
        case: None,
        analysis: Some(AnalysisExport {
            run_id: "11111111-1111-4111-8111-111111111111".into(),
            layout: AnalysisLayout::Aggregates,
            encoding: AnalysisEncoding::Csv,
        }),
    };
    let v = round_trip(&req);
    assert_eq!(v["analysis"]["layout"], "aggregates");
    validates_as("ExportRequest", &v);
    // Older requests carry no `analysis` key and still decode.
    let mut old = v.clone();
    old.as_object_mut().unwrap().remove("analysis");
    old["format"] = json!("csv");
    let decoded: ExportRequest = serde_json::from_value(old).unwrap();
    assert!(decoded.analysis.is_none());
    assert_eq!(
        serde_json::to_value(&decoded).unwrap().get("analysis"),
        None,
        "absent stays absent"
    );
}
