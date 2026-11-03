//! owner: WP-X.8
//!
//! `analysis_results` exports (RUN-7, PRV-3, PRV-5): request rules (de-identified only,
//! research or quality improvement, the run succeeded and is the org's), both layouts in
//! every encoding free of identifiers, suppressed cells without numbers, the manifest
//! provenance equal to the run row, Parquet metadata `curv.*`, the exported sessions in
//! `export_items`, and withdrawn participants left out. The fixture inserts the finished
//! `analysis_runs` row and its sealed rows object directly (WP-X.3 writes the same shapes).
//! Needs `DATABASE_URL`.

mod common;
mod export_support;
mod participant_support;

use asd_backend::crypto::Aad;
use asd_contract::api_v1::Role;
use bytes::Bytes;
use common::auth::seed_org;
use export_support::{body, unzip};
use http::StatusCode;
use participant_support::*;
use serde_json::{json, Value};
use uuid::Uuid;

const DEVICE: &str = "7c1f8a90-aaaa-4bbb-8ccc-0123456789ab";

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn dist(n: u32, mean: f64) -> Value {
    json!({"n": n, "suppressed": false, "mean": mean, "sd": 0.1, "median": mean,
           "iqr": [mean - 0.1, mean + 0.1], "p10": mean - 0.2, "p90": mean + 0.2,
           "ci95_mean": [mean - 0.05, mean + 0.05], "hist": null})
}

fn cmp(n: u32, mean: f64) -> Value {
    json!({"published": dist(n, mean), "draft": dist(n, mean + 0.01), "delta": dist(n, 0.01)})
}

fn hidden() -> Value {
    json!({"n": null, "suppressed": true, "mean": null, "sd": null, "median": null,
           "iqr": null, "p10": null, "p90": null, "ci95_mean": null, "hist": null})
}

fn summary() -> Value {
    let band = |name: &str, n: u32| {
        json!({"band": name, "n": n, "suppressed": false,
               "metrics": {"eye_dwell": cmp(n, 0.5)}, "composite": cmp(n, 0.6),
               "flag_rate": {"published": null, "draft": null}})
    };
    let mut small = band("36-48", 0);
    small["n"] = Value::Null;
    small["suppressed"] = json!(true);
    small["metrics"] =
        json!({"eye_dwell": {"published": hidden(), "draft": hidden(), "delta": hidden()}});
    small["composite"] = json!({"published": hidden(), "draft": hidden(), "delta": hidden()});
    json!({
        "counts": {"sessions": 3, "by_level": {"frames": 0, "score": 3}, "excluded": {},
                   "params_not_applied": {}, "suppressed_cells": 1},
        "flags": {"published": {}, "draft": {}, "transitions": []},
        "quality": {"transitions": []},
        "by_age_band": [band("24-36", 20), small],
        "overall": band("overall", 20),
        "small_cell_k": 12,
        "includes_synthetic": false
    })
}

fn row(session: &str, device: Option<&str>) -> Value {
    let side = |c: f64| {
        json!({"composite": c, "flag": "yellow", "quality_status": "ok",
                               "metrics": {"eye_dwell": 0.5}})
    };
    json!({"session_id": session, "level": "score", "age_months": 30, "age_band": "24-36",
           "device_id": device, "published": side(0.6), "draft": side(0.65),
           "delta": {"composite": 0.05, "metrics": {"eye_dwell": 0.0}}, "warnings": []})
}

struct Run {
    id: Uuid,
}

impl Ctx {
    /// A finished run of `org` with the given sealed rows (none: no result object).
    async fn run(&self, org: Uuid, status: &str, rows: Option<Vec<Value>>) -> Run {
        let id = Uuid::new_v4();
        let key = rows
            .as_ref()
            .map(|_| format!("analysis/{org}/{id}/sessions.jsonl.sealed"));
        if let Some(rows) = rows {
            let plain: String = rows.iter().map(|r| format!("{r}\n")).collect();
            let keyring = self.state.keyring.clone().expect("keyring");
            let sealed = keyring
                .encrypt(
                    org,
                    &Aad::new("analysis_runs", "result", org, id),
                    plain.as_bytes(),
                )
                .await
                .unwrap();
            let mut blob = sealed.key_id.as_bytes().to_vec();
            blob.extend_from_slice(&sealed.ciphertext);
            self.state
                .blobs
                .put(
                    key.as_deref().unwrap(),
                    "application/octet-stream",
                    Bytes::from(blob),
                )
                .await
                .unwrap();
        }
        let h = |b: u8| vec![b; 32];
        sqlx::query(
            "INSERT INTO analysis_runs
               (id, org_id, kind, status, definition, definition_hash, cohort_hash,
                membership_hash, profile_hash, published_profile_version, baselines_version,
                catalog_version, bundle_version, core_version, contract_version,
                pipeline_version, registry_version, seed, level_requested, sessions_n,
                summary, result_blob_key, result_bytes, finished_at)
             VALUES ($1, $2, 'cohort_rescore', $3, '{}', $4, $5, $6, $7, '1.2.0-pub', 'b-7',
                     'cat-3', 'bundle-9', '0.9.1', '1.2.0', 'gaze-4', 'reg-2', 42, 'auto', 3,
                     $8, $9, $10, now())",
        )
        .bind(id)
        .bind(org)
        .bind(status)
        .bind(h(0xab))
        .bind(h(0xc0))
        .bind(h(0x3d))
        .bind(h(0x9e))
        .bind(summary())
        .bind(key.as_deref())
        .bind(key.as_ref().map(|_| 100i32))
        .execute(&self.db.pool)
        .await
        .unwrap();
        Run { id }
    }

    /// Three exportable sessions: two consented participants' (one has the device) and a
    /// participant who withdrew after the run.
    async fn analysis_cohort(&self) -> Vec<Value> {
        let a = self.participant(self.org, "TOKEN-ALPHA").await;
        let w = self.participant(self.org, "TOKEN-WITHDRAWN").await;
        self.exportable_session(
            a,
            "TOKEN-ALPHA",
            "SESS-ONE",
            "2026-10-01T10:00:00Z",
            Some(true),
        )
        .await;
        self.exportable_session(
            a,
            "TOKEN-ALPHA",
            "SESS-TWO",
            "2026-09-03T10:00:00Z",
            Some(true),
        )
        .await;
        self.exportable_session(
            w,
            "TOKEN-WITHDRAWN",
            "SESS-THREE",
            "2026-10-02T10:00:00Z",
            Some(true),
        )
        .await;
        self.withdraw(w).await;
        vec![
            row("SESS-ONE", Some(DEVICE)),
            row("SESS-TWO", None),
            row("SESS-THREE", None),
        ]
    }
}

fn request(run: &Run, layout: &str, encoding: &str) -> Value {
    let mut b = body("analysis_results", "deidentified", "research");
    b["analysis"] = json!({"run_id": run.id, "layout": layout, "encoding": encoding});
    b
}

const FORBIDDEN: &[&str] = &[
    "SESS-ONE",
    "SESS-TWO",
    "SESS-THREE",
    "TOKEN-ALPHA",
    "TOKEN-WITHDRAWN",
    DEVICE,
    "0.777",
];

fn scan(name: &str, bytes: &[u8]) {
    let text = String::from_utf8_lossy(bytes);
    for bad in FORBIDDEN {
        assert!(!text.contains(bad), "{name} contains {bad}");
    }
}

#[tokio::test]
async fn request_rules_follow_run_7() {
    let Some(c) = Ctx::new().await else { return };
    let rows = c.analysis_cohort().await;
    let run = c.run(c.org, "succeeded", Some(rows.clone())).await;
    let (_, researcher) = c.token(Role::Researcher).await;
    let expect = |want: StatusCode, (s, b): (StatusCode, Value)| assert_eq!(s, want, "{b}");

    // Identified mode is refused (the role may not even try it: 403 or 400, never queued).
    let (_, clin) = c.token(Role::Clinician).await;
    let mut identified = request(&run, "aggregates", "jsonl");
    identified["mode"] = json!("identified");
    identified["purpose"] = json!("clinical_care");
    let (s, b) = c.request_export(&clin, identified).await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{b}");
    let mut identified = request(&run, "aggregates", "jsonl");
    identified["mode"] = json!("identified");
    expect(
        StatusCode::BAD_REQUEST,
        c.request_export(&clin, identified).await,
    );

    // Purpose: research or quality improvement only.
    let mut care = request(&run, "aggregates", "jsonl");
    care["purpose"] = json!("clinical_care");
    expect(
        StatusCode::BAD_REQUEST,
        c.request_export(&researcher, care).await,
    );
    let mut qi = request(&run, "aggregates", "jsonl");
    qi["purpose"] = json!("quality_improvement");
    expect(
        StatusCode::ACCEPTED,
        c.request_export(&researcher, qi).await,
    );

    // Block missing, parquet for aggregates, junk run id.
    expect(
        StatusCode::BAD_REQUEST,
        c.request_export(
            &researcher,
            body("analysis_results", "deidentified", "research"),
        )
        .await,
    );
    expect(
        StatusCode::BAD_REQUEST,
        c.request_export(&researcher, request(&run, "aggregates", "parquet"))
            .await,
    );
    let mut junk = request(&run, "sessions", "csv");
    junk["analysis"]["run_id"] = json!("nope");
    expect(
        StatusCode::BAD_REQUEST,
        c.request_export(&researcher, junk).await,
    );
    // The analysis block belongs to analysis_results only.
    let mut other = body("csv", "deidentified", "research");
    other["analysis"] = json!({"run_id": run.id, "layout": "sessions", "encoding": "csv"});
    expect(
        StatusCode::BAD_REQUEST,
        c.request_export(&researcher, other).await,
    );

    // A run that has not succeeded: 400; another org's run and an unknown run: 404.
    let running = c.run(c.org, "running", None).await;
    expect(
        StatusCode::BAD_REQUEST,
        c.request_export(&researcher, request(&running, "aggregates", "csv"))
            .await,
    );
    let org_b = seed_org(&c.db.pool, "Clinic B").await;
    let theirs = c.run(org_b, "succeeded", None).await;
    expect(
        StatusCode::NOT_FOUND,
        c.request_export(&researcher, request(&theirs, "aggregates", "csv"))
            .await,
    );
    let unknown = Run { id: Uuid::new_v4() };
    expect(
        StatusCode::NOT_FOUND,
        c.request_export(&researcher, request(&unknown, "aggregates", "csv"))
            .await,
    );
}

#[tokio::test]
async fn aggregates_carry_the_run_provenance_and_no_numbers_for_suppressed_cells() {
    let Some(c) = Ctx::new().await else { return };
    let rows = c.analysis_cohort().await;
    let run = c.run(c.org, "succeeded", Some(rows)).await;
    let (_, researcher) = c.token(Role::Researcher).await;

    let done = c
        .export_and_run(&researcher, request(&run, "aggregates", "csv"))
        .await;
    assert_eq!(done["status"], "succeeded", "{done}");
    assert_eq!(done["content_type"], "application/zip");
    assert_eq!(done["filters"], json!({}));
    assert!(c
        .export_items(done["id"].as_str().unwrap())
        .await
        .is_empty());
    let bytes = c.artifact(done["id"].as_str().unwrap()).await;
    let entries = unzip::entries(&bytes);
    let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
    assert_eq!(names, ["aggregates.csv", "manifest.json", "README.txt"]);
    for e in &entries {
        scan(&e.name, &e.data);
    }
    let csv = String::from_utf8(entries[0].data.clone()).unwrap();
    let id = run.id;
    assert!(
        csv.contains(&format!(
            "{id},36-48,composite,published,,,,,,,,,,,true\r\n"
        )),
        "{csv}"
    );
    assert!(
        csv.contains(&format!("{id},24-36,composite,published,20,0.6,")),
        "{csv}"
    );

    // Manifest provenance equals the run row.
    let manifest: Value = serde_json::from_slice(&entries[1].data).unwrap();
    let p = &manifest["provenance"];
    let row: (Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>, i64) = sqlx::query_as(
        "SELECT definition_hash, cohort_hash, membership_hash, profile_hash, seed
         FROM analysis_runs WHERE id = $1",
    )
    .bind(run.id)
    .fetch_one(&c.db.pool)
    .await
    .unwrap();
    assert_eq!(p["run_id"], run.id.to_string());
    assert_eq!(p["definition_hash"], hex(&row.0));
    assert_eq!(p["cohort_hash"], hex(&row.1));
    assert_eq!(p["membership_hash"], hex(&row.2));
    assert_eq!(p["profile_hash"], hex(&row.3));
    assert_eq!(p["seed"], row.4);
    for (k, v) in [
        ("published_profile_version", "1.2.0-pub"),
        ("baselines_version", "b-7"),
        ("catalog_version", "cat-3"),
        ("bundle_version", "bundle-9"),
        ("core_version", "0.9.1"),
        ("contract_version", "1.2.0"),
        ("pipeline_version", "gaze-4"),
        ("registry_version", "reg-2"),
        ("level_requested", "auto"),
    ] {
        assert_eq!(p[k], v, "{k}");
    }

    // JSONL aggregates: manifest line then rows, same scan.
    let jsonl = c
        .export_and_run(&researcher, request(&run, "aggregates", "jsonl"))
        .await;
    assert_eq!(jsonl["status"], "succeeded", "{jsonl}");
    let bytes = c.artifact(jsonl["id"].as_str().unwrap()).await;
    scan("aggregates.jsonl", &bytes);
    let first: Value =
        serde_json::from_slice(bytes.split(|b| *b == b'\n').next().unwrap()).unwrap();
    assert_eq!(first["provenance"], *p);
}

#[tokio::test]
async fn sessions_layout_is_deidentified_in_every_encoding() {
    let Some(c) = Ctx::new().await else { return };
    let rows = c.analysis_cohort().await;
    let run = c.run(c.org, "succeeded", Some(rows)).await;
    let (_, researcher) = c.token(Role::Researcher).await;

    for encoding in ["jsonl", "csv", "parquet"] {
        let done = c
            .export_and_run(&researcher, request(&run, "sessions", encoding))
            .await;
        assert_eq!(done["status"], "succeeded", "{encoding}: {done}");
        // The withdrawn participant's session is left out; the other two are exported and
        // accounted for.
        assert_eq!(done["row_count"], 2, "{encoding}");
        assert_eq!(done["excluded"], json!({"withdrawn": 1}), "{encoding}");
        let id = done["id"].as_str().unwrap();
        assert_eq!(c.export_items(id).await, ["SESS-ONE", "SESS-TWO"]);
        let bytes = c.artifact(id).await;
        match encoding {
            "csv" => {
                let entries = unzip::entries(&bytes);
                assert_eq!(
                    entries.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(),
                    ["sessions.csv", "manifest.json", "README.txt"]
                );
                for e in &entries {
                    scan(&e.name, &e.data);
                }
                let csv = String::from_utf8(entries[0].data.clone()).unwrap();
                assert!(
                    csv.contains(",2026-10,") && csv.contains(",2026-09,"),
                    "{csv}"
                );
                assert!(csv.lines().nth(1).unwrap().starts_with("S-"));
            }
            "parquet" => {
                scan("sessions.parquet", &bytes);
                let text = String::from_utf8_lossy(&bytes);
                // Footer key-value metadata (uncompressed): run id and definition hash.
                assert!(text.contains("curv.run_id") && text.contains(&run.id.to_string()));
                assert!(text.contains("curv.definition_hash") && text.contains(&"ab".repeat(32)));
                assert!(bytes.starts_with(b"PAR1"));
            }
            _ => {
                scan("sessions.jsonl", &bytes);
                let text = String::from_utf8(bytes).unwrap();
                assert!(text.contains("\"site_pseudonym\":\"D-"));
                assert!(!text.contains("age_months"));
            }
        }
    }

    // The audit row names counts, never ids; the export row stores no session id.
    let audit = c.audit_dump().await;
    for bad in ["SESS-ONE", "TOKEN-ALPHA", DEVICE] {
        assert!(!audit.contains(bad), "audit holds {bad}");
    }
    assert_eq!(c.audit_rows("export.complete").await.len(), 3);
}

/// PRV-5: the backend's analysis build never reads the research log.
#[test]
fn analysis_build_does_not_read_the_research_log() {
    let src = include_str!("../src/export_build/analysis.rs");
    for forbidden in ["research_frames", "research.jsonl", "open_research_log"] {
        // The module docs may name the log once to say it is never read.
        let code: String = src
            .lines()
            .filter(|l| !l.trim_start().starts_with("//"))
            .collect();
        assert!(!code.contains(forbidden), "{forbidden}");
    }
}
