//! owner: WP-X.2
//!
//! Cohorts (COH-1..3, PRV-1, AGE-3; acceptance 9): the preview over a hand-built fixture of
//! 30 sessions across the default bands, with `k = 12` and `k = 5`; band, date and
//! `require_frames` filters; research consent and withdrawal always applied; bands from an
//! experiment's profile; CRUD with RBAC and audits. Needs `DATABASE_URL`; skipped otherwise.

mod common;
mod research_support;

use asd_contract::api_v1::Role;
use asd_contract::problem;
use http::{Method, StatusCode};
use research_support::*;
use serde_json::{json, Value};

/// The fixture. In the cohort (`tracking_verdicts: [good, fair]`): 7 sessions in 12-17 (3
/// with a research log), 13 in 24-35 (5 with a log, 2 of them received 400 days ago), 10 in
/// 36-47 (no log): 30. Left out: 2 without research consent, 1 withdrawn, 1 unscorable, 2
/// without a placeable age (none, 70 months), 2 on quality (poor tracking); not counted at
/// all: a synthetic session and another org's session.
async fn fixture(c: &Ctx) {
    let mut n = 0;
    let mut add = |age: i32, frames: bool| {
        n += 1;
        let mut s = Seed::new(&format!("S-{n:02}"), age);
        s.frames = frames;
        s
    };
    let mut seeds = Vec::new();
    for i in 0..7 {
        seeds.push(add(12 + i % 6, i < 3));
    }
    for i in 0..13 {
        let mut s = add(24 + i % 12, i < 5);
        if i >= 11 {
            s.days_ago = 400;
        }
        seeds.push(s);
    }
    for i in 0..10 {
        seeds.push(add(36 + i, false));
    }
    let mut no_consent = Seed::new("X-NOCONSENT", 30);
    no_consent.consented = Some(false);
    let mut no_opt = Seed::new("X-NOOPT", 30);
    no_opt.consented = None;
    let withdrawn = Seed::new("X-WITHDRAWN", 30);
    let mut unscorable = Seed::new("X-UNSCORABLE", 30);
    unscorable.scoring_status = Some("unscorable".into());
    let mut no_age = Seed::new("X-NOAGE", 30);
    no_age.age_months = None;
    let old_child = Seed::new("X-70", 70);
    let mut poor = Seed::new("X-POOR", 30);
    poor.tracking = Some("poor".into());
    let mut untracked = Seed::new("X-UNTRACKED", 40);
    untracked.tracking = None;
    let mut synthetic = Seed::new("X-SYN", 30);
    synthetic.capture_source = "synthetic".into();
    seeds.extend([
        no_consent, no_opt, withdrawn, unscorable, no_age, old_child, poor, untracked, synthetic,
    ]);
    for s in &seeds {
        c.seed(c.org, s).await;
    }
    c.withdraw(c.org, "TOK-X-WITHDRAWN").await;
    c.seed(c.other, &Seed::new("O-1", 30)).await;
}

async fn preview(c: &Ctx, def: Value, bearer: &str) -> Value {
    let (s, b) = c
        .call(
            Method::POST,
            &format!("/v1/orgs/{}/cohorts/preview", c.org),
            Some(def),
            bearer,
        )
        .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    b
}

fn bands(p: &Value) -> Vec<(String, Value, Value)> {
    p["by_age_band"]
        .as_array()
        .unwrap()
        .iter()
        .map(|b| {
            (
                b["band"].as_str().unwrap().to_owned(),
                b["n"].clone(),
                b["frames_n"].clone(),
            )
        })
        .collect()
}

fn band(id: &str, n: Value, frames: Value) -> (String, Value, Value) {
    (id.to_owned(), n, frames)
}

#[tokio::test]
async fn preview_counts_match_the_fixture_with_k_12_and_k_5() {
    let Some(c) = Ctx::new().await else { return };
    fixture(&c).await;
    let (_, researcher) = c.token(Role::Researcher).await;
    let (_, owner) = c.token(Role::Owner).await;
    let def = json!({"tracking_verdicts": ["good", "fair"]});

    let p = preview(&c, def.clone(), &researcher).await;
    assert_eq!(p["small_cell_k"], 12);
    assert_eq!(p["n"], 30);
    assert_eq!(p["by_level"], json!({"frames": 8, "score": 22}));
    assert_eq!(
        p["excluded"],
        json!({"no_consent": 2, "withdrawn": 1, "unscorable": 1, "no_age": 2, "quality": 2})
    );
    assert_eq!(
        bands(&p),
        [
            band("12-17", Value::Null, Value::Null),
            band("18-23", json!(0), json!(0)),
            band("24-35", json!(13), Value::Null),
            band("36-47", Value::Null, Value::Null),
            band("48-60", json!(0), json!(0)),
        ]
    );
    assert_eq!(p["by_age_band"][0]["suppressed"], true);
    assert_eq!(p["by_age_band"][1]["suppressed"], false);

    // k = 5 (org setting, validated 5..=50): the 7- and 10-session bands show.
    let (s, b) = c.set_small_cell_k(4, &owner).await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{b}");
    let (s, b) = c.set_small_cell_k(51, &owner).await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{b}");
    let (s, b) = c.set_small_cell_k(5, &owner).await;
    assert_eq!(s, StatusCode::OK, "{b}");
    assert_eq!(b["settings"]["research"]["small_cell_k"], 5);
    let p = preview(&c, def.clone(), &researcher).await;
    assert_eq!(p["small_cell_k"], 5);
    assert_eq!(
        bands(&p),
        [
            band("12-17", json!(7), Value::Null),
            band("18-23", json!(0), json!(0)),
            band("24-35", json!(13), json!(5)),
            band("36-47", json!(10), json!(0)),
            band("48-60", json!(0), json!(0)),
        ]
    );

    // Bands, a date window and `require_frames`.
    let mut d = def.clone();
    d["age_bands"] = json!(["24-35", "36-47"]);
    let p = preview(&c, d, &researcher).await;
    assert_eq!(p["n"], 23);
    assert_eq!(
        bands(&p),
        [
            band("24-35", json!(13), json!(5)),
            band("36-47", json!(10), json!(0))
        ]
    );
    // Out-of-selection sessions are not "excluded"; age-less ones still are.
    assert_eq!(p["excluded"]["no_consent"], 2);
    assert_eq!(p["excluded"]["no_age"], 2);
    let from = (chrono::Utc::now() - chrono::Duration::days(365))
        .format("%Y-%m-%d")
        .to_string();
    let mut d = def.clone();
    d["from"] = json!(from);
    let p = preview(&c, d, &researcher).await;
    assert_eq!(p["n"], 28);
    assert_eq!(p["by_age_band"][2]["n"], 11);
    let mut d = def.clone();
    d["to"] = json!(from);
    let p = preview(&c, d, &researcher).await;
    assert_eq!(p["n"], Value::Null, "2 sessions: suppressed even at k = 5");
    let mut d = def.clone();
    d["require_frames"] = json!(true);
    let p = preview(&c, d, &researcher).await;
    assert_eq!(p["n"], 8);
    assert_eq!(p["by_level"], json!({"frames": 8, "score": 0}));
    assert_eq!(bands(&p)[0], band("12-17", Value::Null, Value::Null));
    assert_eq!(bands(&p)[2], band("24-35", json!(5), json!(5)));
    // Quality filters: a valid-ratio floor above everyone's 0.9 leaves nobody.
    let p = preview(&c, json!({"min_valid_ratio": 0.95}), &researcher).await;
    assert_eq!(p["n"], 0);
    // Without a tracking filter the poor and untracked sessions are in.
    let p = preview(&c, json!({}), &researcher).await;
    assert_eq!(p["n"], 32);
    assert_eq!(p["excluded"]["quality"], 0);
    // Unscorable sessions in on request.
    let p = preview(
        &c,
        json!({"scoring_status": ["scored", "unscorable"]}),
        &researcher,
    )
    .await;
    assert_eq!(p["n"], 33);

    // Research consent and withdrawal are not choices: unknown members are refused.
    let (s, _) = c
        .call(
            Method::POST,
            &format!("/v1/orgs/{}/cohorts/preview", c.org),
            Some(json!({"include_unconsented": true})),
            &researcher,
        )
        .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    // Synthetic data needs ASD_ALLOW_SYNTHETIC; unknown band ids name their member.
    for (bad, member) in [
        (
            json!({"include_synthetic": true}),
            "definition.include_synthetic",
        ),
        (json!({"age_bands": ["6-11"]}), "definition.age_bands[0]"),
        (json!({"from": "yesterday"}), "definition.from"),
    ] {
        let (s, b) = c
            .call(
                Method::POST,
                &format!("/v1/orgs/{}/cohorts/preview", c.org),
                Some(bad),
                &researcher,
            )
            .await;
        assert_eq!(s, StatusCode::BAD_REQUEST, "{b}");
        assert_eq!(b["invalid_params"][0]["name"], member, "{b}");
    }
    c.db.cleanup().await;
}

#[tokio::test]
async fn preview_uses_an_experiments_bands_and_rbac() {
    let Some(c) = Ctx::new().await else { return };
    fixture(&c).await;
    let (_, researcher) = c.token(Role::Researcher).await;
    let (_, viewer) = c.token(Role::Viewer).await;
    let (_, clinician) = c.token(Role::Clinician).await;
    let (_, stranger) = c.token_in(Some(c.other), Role::Researcher).await;
    let (_, e) = c
        .call(
            Method::POST,
            &format!("/v1/orgs/{}/experiments", c.org),
            Some(json!({"name": "Two bands"})),
            &researcher,
        )
        .await;
    let id = e["id"].as_str().unwrap();
    let mut p = e["profile"].clone();
    p["ageBands"] = json!([
        {"id": "young", "minMonths": 0, "maxMonths": 29},
        {"id": "old", "minMonths": 30, "maxMonths": 240}
    ]);
    let (s, b) = c
        .patch_if(
            &format!("/v1/experiments/{id}"),
            json!({"profile": p}),
            Some("\"1\""),
            &researcher,
        )
        .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    let uri = format!("/v1/orgs/{}/cohorts/preview?experiment_id={id}", c.org);
    let (s, prev) = c
        .call(
            Method::POST,
            &uri,
            Some(json!({"tracking_verdicts": ["good", "fair"]})),
            &researcher,
        )
        .await;
    assert_eq!(s, StatusCode::OK, "{prev}");
    let names: Vec<&str> = prev["by_age_band"]
        .as_array()
        .unwrap()
        .iter()
        .map(|b| b["band"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["young", "old"]);
    // 70 months is placeable now: 31 in, one age-less.
    assert_eq!(prev["n"], 31);
    assert_eq!(prev["excluded"]["no_age"], 1);
    // AnalysisRead: clinician yes, viewer no; another org's experiment is a 404.
    let (s, _) = c
        .call(Method::POST, &uri, Some(json!({})), &clinician)
        .await;
    assert_eq!(s, StatusCode::OK);
    let (s, _) = c.call(Method::POST, &uri, Some(json!({})), &viewer).await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    let (s, _) = c
        .call(
            Method::POST,
            &format!("/v1/orgs/{}/cohorts/preview?experiment_id={id}", c.other),
            Some(json!({})),
            &stranger,
        )
        .await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    c.db.cleanup().await;
}

#[tokio::test]
async fn cohort_crud_audits_and_rbac() {
    let Some(c) = Ctx::new().await else { return };
    let (researcher_id, researcher) = c.token(Role::Researcher).await;
    let (_, clinician) = c.token(Role::Clinician).await;
    let (_, root) = c.token(Role::Superadmin).await;
    let (_, stranger) = c.token_in(Some(c.other), Role::Owner).await;
    let uri = format!("/v1/orgs/{}/cohorts", c.org);
    let def = json!({"age_bands": ["24-35", "36-47"], "tracking_verdicts": ["good", "fair"],
                     "from": "2025-11-01"});
    let (s, made) = c
        .call(
            Method::POST,
            &uri,
            Some(json!({"name": "24-47 months, good/fair", "definition": def})),
            &researcher,
        )
        .await;
    assert_eq!(s, StatusCode::CREATED, "{made}");
    assert_eq!(made["created_by"], researcher_id.to_string());
    assert_eq!(made["definition"]["age_bands"], json!(["24-35", "36-47"]));
    assert_eq!(made["definition"]["require_frames"], false);
    assert_eq!(made["definition_hash"].as_str().unwrap().len(), 64);
    let id = made["id"].as_str().unwrap().to_owned();
    let (s, b) = c
        .call(
            Method::POST,
            &uri,
            Some(json!({"name": "24-47 months, good/fair", "definition": {}})),
            &researcher,
        )
        .await;
    assert_eq!(s, StatusCode::CONFLICT, "{b}");
    assert_eq!(
        b["type"],
        problem::type_uri(problem::COHORT_NAME_EXISTS),
        "{b}"
    );
    for bearer in [&clinician, &root] {
        let (s, _) = c
            .call(
                Method::POST,
                &uri,
                Some(json!({"name": "x", "definition": {}})),
                bearer,
            )
            .await;
        assert_eq!(s, StatusCode::FORBIDDEN);
    }
    let (s, page) = c.call(Method::GET, &uri, None, &clinician).await;
    assert_eq!(s, StatusCode::OK, "{page}");
    assert_eq!(page["items"].as_array().unwrap().len(), 1);
    let (s, _) = c
        .call(Method::GET, &format!("/v1/cohorts/{id}"), None, &stranger)
        .await;
    assert_eq!(s, StatusCode::NOT_FOUND);

    // Update the definition: a new hash; then archive (listed only with ?archived=true).
    let (s, upd) = c
        .call(
            Method::PATCH,
            &format!("/v1/cohorts/{id}"),
            Some(json!({"definition": {"require_frames": true}})),
            &researcher,
        )
        .await;
    assert_eq!(s, StatusCode::OK, "{upd}");
    assert_ne!(upd["definition_hash"], made["definition_hash"]);
    let (s, arch) = c
        .call(
            Method::PATCH,
            &format!("/v1/cohorts/{id}"),
            Some(json!({"archived": true})),
            &researcher,
        )
        .await;
    assert_eq!(s, StatusCode::OK, "{arch}");
    assert!(arch["archived_at"].is_string());
    let (_, page) = c.call(Method::GET, &uri, None, &researcher).await;
    assert!(page["items"].as_array().unwrap().is_empty());
    let (_, page) = c
        .call(
            Method::GET,
            &format!("{uri}?archived=true"),
            None,
            &researcher,
        )
        .await;
    assert_eq!(page["items"].as_array().unwrap().len(), 1);
    assert_eq!(c.audit_rows("cohort.create").await.len(), 1);
    assert_eq!(c.audit_rows("cohort.update").await.len(), 1);
    assert_eq!(c.audit_rows("cohort.archive").await.len(), 1);

    // Renaming another cohort onto a taken name (archived ones included) is the same 409.
    let (s, other) = c
        .call(
            Method::POST,
            &uri,
            Some(json!({"name": "other", "definition": {}})),
            &researcher,
        )
        .await;
    assert_eq!(s, StatusCode::CREATED, "{other}");
    let (s, b) = c
        .call(
            Method::PATCH,
            &format!("/v1/cohorts/{}", other["id"].as_str().unwrap()),
            Some(json!({"name": "24-47 months, good/fair"})),
            &researcher,
        )
        .await;
    assert_eq!(s, StatusCode::CONFLICT, "{b}");
    assert_eq!(
        b["type"],
        problem::type_uri(problem::COHORT_NAME_EXISTS),
        "{b}"
    );
    c.db.cleanup().await;
}
