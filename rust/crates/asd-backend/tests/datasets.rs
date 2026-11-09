//! owner: WP-X.11
//!
//! Research datasets (ADR-0015 R-7..R-9): freezing from a cohort applies the exclusion rules in
//! order (no age, quality gates, explicit list), assigns a seeded split stratified by age band,
//! attaches signed review outcomes as labels with pseudonymous reviewers, records lineage and
//! the data dictionary, and the content hash recomputes from the response and repeats for the
//! same inputs; members page with one audit per page; rows are immutable (triggers); erasure
//! marks a member `erased` and keeps the hash; runs take their membership from the snapshot;
//! validation refusals and the RBAC rows. Needs `DATABASE_URL`; skipped otherwise.

mod analysis_support;
mod common;
mod research_support;

use asd_contract::api_v1::Role;
use asd_contract::datasets::{
    content_hash, DatasetMember, Exclusion, ExclusionRule, Lineage, Split,
};
use http::{Method, StatusCode};
use research_support::*;
use serde_json::{json, Value};
use uuid::Uuid;

async fn create_cohort(c: &Ctx, bearer: &str, name: &str, definition: Value) -> String {
    let (s, b) = c
        .call(
            Method::POST,
            &format!("/v1/orgs/{}/cohorts", c.org),
            Some(json!({"name": name, "definition": definition})),
            bearer,
        )
        .await;
    assert_eq!(s, StatusCode::CREATED, "{b}");
    b["id"].as_str().unwrap().to_owned()
}

async fn create(c: &Ctx, body: Value, bearer: &str) -> (StatusCode, Value) {
    c.call(Method::POST, "/v1/research/datasets", Some(body), bearer)
        .await
}

/// Every member of `id`, `limit` per page; returns the rows and the page count.
async fn all_members(c: &Ctx, id: &str, limit: u32, bearer: &str) -> (Vec<Value>, usize) {
    let mut uri = format!("/v1/research/datasets/{id}/members?limit={limit}");
    let mut rows = Vec::new();
    let mut pages = 0;
    loop {
        let (s, page) = c.call(Method::GET, &uri, None, bearer).await;
        assert_eq!(s, StatusCode::OK, "{page}");
        pages += 1;
        rows.extend(page["items"].as_array().unwrap().iter().cloned());
        match page["next_cursor"].as_str() {
            Some(cur) => {
                uri = format!("/v1/research/datasets/{id}/members?limit={limit}&cursor={cur}")
            }
            None => break,
        }
    }
    (rows, pages)
}

/// 30 admitted sessions (20 in 24-35, 10 in 36-47) and three the rules will catch.
async fn fixture(c: &Ctx) {
    for i in 0..20 {
        c.seed(c.org, &Seed::new(&format!("S-{i:02}"), 24 + i % 12))
            .await;
    }
    for i in 0..10 {
        c.seed(c.org, &Seed::new(&format!("S-{}", 20 + i), 36 + i))
            .await;
    }
    let mut no_age = Seed::new("X-NOAGE", 30);
    no_age.age_months = None;
    let mut low = Seed::new("X-LOW", 30);
    low.valid = 0.4;
    let mut poor = Seed::new("X-POOR", 30);
    poor.tracking = Some("poor".into());
    for s in [no_age, low, poor] {
        c.seed(c.org, &s).await;
    }
    c.seed(c.other, &Seed::new("O-1", 30)).await;
}

/// A signed clinical review of `session` with `disposition`, signed by `signer`.
async fn signed_review(c: &Ctx, session: &str, disposition: &str, signer: Uuid) {
    let pool = &c.db.pool;
    let review: Uuid = sqlx::query_scalar(
        "INSERT INTO clinical_reviews (org_id, session_id, state) VALUES ($1, $2, 'final')
         RETURNING id",
    )
    .bind(c.org)
    .bind(session)
    .fetch_one(pool)
    .await
    .unwrap();
    let version: Uuid = sqlx::query_scalar(
        "INSERT INTO clinical_review_versions
           (review_id, org_id, version_no, status, disposition, content_hash, signed_at)
         VALUES ($1, $2, 1, 'signed', $3, decode(repeat('ab', 32), 'hex'), now())
         RETURNING id",
    )
    .bind(review)
    .bind(c.org)
    .bind(disposition)
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO clinical_signatures (org_id, review_id, version_id, signer_id, role,
             meaning, content_hash, signer_display_name)
         VALUES ($1, $2, $3, $4, 'sign', 'reviewed', decode(repeat('ab', 32), 'hex'), 'Dr. Test')",
    )
    .bind(c.org)
    .bind(review)
    .bind(version)
    .bind(signer)
    .execute(pool)
    .await
    .unwrap();
}

fn body(name: &str, cohort: &str, seed: i64) -> Value {
    json!({
        "name": name,
        "description": "first freeze",
        "cohort_id": cohort,
        "exclusion_rules": [
            {"kind": "quality_gates", "id": "quality", "min_valid_ratio": 0.6,
             "exclude_tracking_verdicts": ["poor"]},
            {"kind": "explicit_list", "id": "manual",
             "sessions": [{"session_id": "S-03", "reason": "recorded twice"}]},
        ],
        "labels": "review_outcomes",
        "split": {"seed": seed},
    })
}

/// Recomputes the content hash from a dataset and its members (what a client can verify).
fn recompute(dataset: &Value, members: &[Value]) -> String {
    let members: Vec<DatasetMember> = members
        .iter()
        .map(|m| serde_json::from_value(m.clone()).unwrap())
        .collect();
    let exclusions: Vec<Exclusion> = serde_json::from_value(dataset["exclusions"].clone()).unwrap();
    let rules: Vec<ExclusionRule> =
        serde_json::from_value(dataset["exclusion_rules"].clone()).unwrap();
    let split: Option<Split> = serde_json::from_value(dataset["split"].clone()).unwrap();
    let lineage: Lineage = serde_json::from_value(dataset["lineage"].clone()).unwrap();
    let labels = serde_json::from_value(dataset["label_source"]["kind"].clone()).ok();
    content_hash(
        &members,
        &exclusions,
        &rules,
        split.as_ref(),
        labels,
        &lineage,
    )
}

#[tokio::test]
async fn freeze_applies_rules_split_labels_lineage_and_hash() {
    let Some(c) = Ctx::new().await else { return };
    fixture(&c).await;
    let (clinician, _) = c.token(Role::Clinician).await;
    signed_review(&c, "S-00", "monitor", clinician).await;
    signed_review(&c, "S-03", "refer_for_evaluation", clinician).await;
    let (_, researcher) = c.token(Role::Researcher).await;
    let cohort = create_cohort(&c, &researcher, "all", json!({})).await;

    let (s, d) = create(&c, body("Toddlers v1", &cohort, 42), &researcher).await;
    assert_eq!(s, StatusCode::CREATED, "{d}");
    assert_eq!(d["status"], "frozen");
    assert_eq!(d["cohort_id"], cohort.as_str());
    assert_eq!(d["member_count"], 29, "{d}");
    assert_eq!(d["erased_count"], 0);
    // Rule order: no age (system), quality gates, explicit list; sorted by session.
    assert_eq!(
        d["exclusions"],
        json!([
            {"session_id": "S-03", "rule_id": "manual", "reason": "recorded twice"},
            {"session_id": "X-LOW", "rule_id": "quality", "reason": "valid_ratio_below"},
            {"session_id": "X-NOAGE", "rule_id": null, "reason": "no_age"},
            {"session_id": "X-POOR", "rule_id": "quality", "reason": "tracking_verdict"},
        ])
    );
    // S-03's label is not counted: the session is excluded.
    assert_eq!(d["label_source"]["kind"], "review_outcomes");
    assert_eq!(d["label_source"]["count"], 1);
    assert_eq!(d["split"]["seed"], 42);
    assert_eq!(d["split"]["strata"], "age_band");
    assert_eq!(
        d["split"]["fractions"],
        json!({"train": 0.7, "validation": 0.15, "holdout": 0.15})
    );
    // Stratified: 19 in 24-35 (S-03 out) and 10 in 36-47, each split by largest remainder.
    let counts = d["split_counts"].as_array().unwrap();
    let band = |b: &str| counts.iter().find(|x| x["stratum"] == b).unwrap().clone();
    assert_eq!(band("24-35")["train"], 13);
    assert_eq!(band("24-35")["validation"], 3);
    assert_eq!(band("24-35")["holdout"], 3);
    assert_eq!(band("36-47")["train"], 7);
    assert_eq!(band("36-47")["validation"], 2);
    assert_eq!(band("36-47")["holdout"], 1);
    let lineage = &d["lineage"];
    assert_eq!(lineage["cohort_id"], cohort.as_str());
    assert_eq!(lineage["profile_version"], "bundled-1.3.0");
    assert!(lineage["code_version"]
        .as_str()
        .unwrap()
        .starts_with("asd-backend/"));
    for f in ["cohort_hash", "profile_hash"] {
        assert_eq!(lineage[f].as_str().unwrap().len(), 64, "{f}");
    }
    assert_eq!(d["data_dictionary"].as_array().unwrap().len(), 9);
    assert_eq!(d["data_dictionary"][2]["unit"], "months");
    let hash = d["content_hash"].as_str().unwrap().to_owned();
    assert_eq!(hash.len(), 64);
    let audits = c.audit_rows("dataset.create").await;
    assert_eq!(audits.len(), 1);
    assert_eq!(audits[0]["after"]["content_hash"], hash.as_str());
    assert_eq!(audits[0]["after"]["excluded"]["quality"], 2);

    // Members: 10 + 10 + 9, one audit per page, pseudonymous, labelled, split.
    let id = d["id"].as_str().unwrap().to_owned();
    let (members, pages) = all_members(&c, &id, 10, &researcher).await;
    assert_eq!(pages, 3);
    assert_eq!(members.len(), 29);
    assert_eq!(c.audit_rows("dataset.members_read").await.len(), 3);
    let s00 = members.iter().find(|m| m["session_id"] == "S-00").unwrap();
    assert_eq!(s00["label"]["value"], "monitor");
    assert_eq!(s00["label"]["source"], "review_outcomes");
    assert!(s00["label"]["reviewer_pseudonym"]
        .as_str()
        .unwrap()
        .starts_with("R-"));
    for m in &members {
        let p = m["participant_pseudonym"].as_str().unwrap();
        assert!(p.starts_with("P-") && p.len() == 14, "{m}");
        assert!(m["split"].as_str().is_some(), "{m}");
        assert!(m.get("participant_token").is_none());
        assert_eq!(m["excluded"], Value::Null);
    }
    // A split filter.
    let (s, train) = c
        .call(
            Method::GET,
            &format!("/v1/research/datasets/{id}/members?split=train&limit=100"),
            None,
            &researcher,
        )
        .await;
    assert_eq!(s, StatusCode::OK, "{train}");
    assert_eq!(train["items"].as_array().unwrap().len(), 20);

    // The hash recomputes from what the API returns (R-7 identity).
    assert_eq!(recompute(&d, &members), hash);

    // Determinism: the same inputs freeze to the same hash and split (pseudonyms differ).
    let (s, again) = create(&c, body("Toddlers v1 again", &cohort, 42), &researcher).await;
    assert_eq!(s, StatusCode::CREATED, "{again}");
    assert_eq!(again["content_hash"], hash.as_str());
    let (members2, _) = all_members(&c, again["id"].as_str().unwrap(), 100, &researcher).await;
    for (a, b) in members.iter().zip(&members2) {
        assert_eq!(a["session_id"], b["session_id"]);
        assert_eq!(a["split"], b["split"]);
        assert_ne!(a["participant_pseudonym"], b["participant_pseudonym"]);
    }
    // Another seed: another assignment, another hash, the same shares.
    let (s, other) = create(&c, body("Toddlers seed 43", &cohort, 43), &researcher).await;
    assert_eq!(s, StatusCode::CREATED, "{other}");
    assert_ne!(other["content_hash"], hash.as_str());
    assert_eq!(other["split_counts"], d["split_counts"]);
    let (members3, _) = all_members(&c, other["id"].as_str().unwrap(), 100, &researcher).await;
    assert!(members
        .iter()
        .zip(&members3)
        .any(|(a, b)| a["split"] != b["split"]));
    // A new version names its parent (lineage) and changes the hash.
    let mut v2 = body("Toddlers v2", &cohort, 42);
    v2["parent_dataset_id"] = json!(id);
    let (s, child) = create(&c, v2, &researcher).await;
    assert_eq!(s, StatusCode::CREATED, "{child}");
    assert_eq!(child["lineage"]["parent_dataset_id"], id.as_str());
    assert_ne!(child["content_hash"], hash.as_str());

    // Immutable after the freeze (migration 0029 triggers).
    let pool = &c.db.pool;
    let uuid: Uuid = id.parse().unwrap();
    assert!(sqlx::query(
        "UPDATE dataset_members SET split = 'train' WHERE dataset_id = $1 AND split <> 'train'"
    )
    .bind(uuid)
    .execute(pool)
    .await
    .is_err());
    assert!(
        sqlx::query("DELETE FROM dataset_members WHERE dataset_id = $1")
            .bind(uuid)
            .execute(pool)
            .await
            .is_err()
    );
    assert!(
        sqlx::query("UPDATE datasets SET member_count = 1 WHERE id = $1")
            .bind(uuid)
            .execute(pool)
            .await
            .is_err()
    );

    // List, detail, archive (once).
    let (s, list) = c
        .call(
            Method::GET,
            "/v1/research/datasets?limit=2",
            None,
            &researcher,
        )
        .await;
    assert_eq!(s, StatusCode::OK, "{list}");
    assert_eq!(list["items"].as_array().unwrap().len(), 2);
    assert!(list["next_cursor"].as_str().is_some());
    let (s, got) = c
        .call(
            Method::GET,
            &format!("/v1/research/datasets/{id}"),
            None,
            &researcher,
        )
        .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(got, d);
    let archive = format!("/v1/research/datasets/{id}/archive");
    let (s, a) = c.call(Method::POST, &archive, None, &researcher).await;
    assert_eq!(s, StatusCode::OK, "{a}");
    assert_eq!(a["status"], "archived");
    assert_eq!(a["content_hash"], hash.as_str());
    let (s, _) = c.call(Method::POST, &archive, None, &researcher).await;
    assert_eq!(s, StatusCode::CONFLICT);
    assert_eq!(c.audit_rows("dataset.archive").await.len(), 1);
    let (s, archived) = c
        .call(
            Method::GET,
            "/v1/research/datasets?status=archived",
            None,
            &researcher,
        )
        .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(archived["items"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn erasure_marks_the_member_and_keeps_the_hash() {
    let Some(c) = Ctx::new().await else { return };
    fixture(&c).await;
    let (_, researcher) = c.token(Role::Researcher).await;
    let (_, admin) = c.token(Role::Admin).await;
    let cohort = create_cohort(&c, &researcher, "all", json!({})).await;
    let (s, d) = create(&c, body("Before erasure", &cohort, 7), &researcher).await;
    assert_eq!(s, StatusCode::CREATED, "{d}");
    let id = d["id"].as_str().unwrap().to_owned();

    let participant = c.participant(c.org, "TOK-S-01").await;
    let (s, b) = c
        .call(
            Method::POST,
            &format!("/v1/orgs/{}/participants/{participant}/erasure", c.org),
            None,
            &admin,
        )
        .await;
    assert!(s.is_success(), "{s} {b}");
    assert!(c.run_jobs().await >= 1);
    let gone: i64 = sqlx::query_scalar("SELECT count(*) FROM sessions WHERE id = 'S-01'")
        .fetch_one(&c.db.pool)
        .await
        .unwrap();
    assert_eq!(gone, 0, "the erasure ran");

    let (s, after) = c
        .call(
            Method::GET,
            &format!("/v1/research/datasets/{id}"),
            None,
            &researcher,
        )
        .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(
        after["content_hash"], d["content_hash"],
        "R-9: identity kept"
    );
    assert_eq!(after["member_count"], 29);
    assert_eq!(after["erased_count"], 1);
    let (members, _) = all_members(&c, &id, 100, &researcher).await;
    assert_eq!(members.len(), 29, "the erased member stays listed");
    let m = members.iter().find(|m| m["session_id"] == "S-01").unwrap();
    assert_eq!(
        *m,
        json!({"session_id": "S-01", "participant_pseudonym": null, "age_months": null,
               "age_band": null, "split": null, "label": null, "excluded": "erased"})
    );
}

#[tokio::test]
async fn refusals_and_rbac() {
    let Some(c) = Ctx::new().await else { return };
    fixture(&c).await;
    let (_, researcher) = c.token(Role::Researcher).await;
    let cohort = create_cohort(&c, &researcher, "all", json!({})).await;
    let invalid = |b: &Value| -> Vec<String> {
        b["invalid_params"]
            .as_array()
            .map(|a| {
                a.iter()
                    .map(|p| p["name"].as_str().unwrap().to_owned())
                    .collect()
            })
            .unwrap_or_default()
    };

    // Rules not applied yet, label import, bad fractions, unknown session, bad cohort.
    let (s, b) = create(
        &c,
        json!({"name": "t", "cohort_id": cohort,
               "exclusion_rules": [{"kind": "tukey", "id": "t", "metric": "eye_dwell_ratio"}]}),
        &researcher,
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{b}");
    assert_eq!(invalid(&b), ["exclusion_rules[0].kind"]);
    let (s, b) = create(
        &c,
        json!({"name": "t", "cohort_id": cohort, "labels": "import",
               "split": {"fractions": {"train": 0.5, "validation": 0.5, "holdout": 0.5}}}),
        &researcher,
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{b}");
    assert_eq!(invalid(&b), ["labels", "split.fractions"]);
    let (s, b) = create(
        &c,
        json!({"name": "t", "cohort_id": cohort, "exclusion_rules": [
            {"kind": "explicit_list", "id": "m",
             "sessions": [{"session_id": "O-1", "reason": "other org"}]}]}),
        &researcher,
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{b}");
    assert_eq!(invalid(&b), ["exclusion_rules[0].sessions[0].session_id"]);
    let (s, b) = create(
        &c,
        json!({"name": "t", "cohort_id": Uuid::new_v4().to_string()}),
        &researcher,
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{b}");
    assert_eq!(invalid(&b), ["cohort_id"]);
    let empty = create_cohort(
        &c,
        &researcher,
        "empty",
        json!({"from": "2000-01-01", "to": "2000-01-02"}),
    )
    .await;
    let (s, b) = create(&c, json!({"name": "t", "cohort_id": empty}), &researcher).await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{b}");
    assert!(b["type"].as_str().unwrap().ends_with("cohort-empty"), "{b}");
    let (s, d) = create(&c, json!({"name": "dup", "cohort_id": cohort}), &researcher).await;
    assert_eq!(s, StatusCode::CREATED, "{d}");
    assert_eq!(d["split"], Value::Null);
    assert_eq!(d["label_source"], Value::Null);
    let (s, _) = create(&c, json!({"name": "dup", "cohort_id": cohort}), &researcher).await;
    assert_eq!(s, StatusCode::CONFLICT);
    let id = d["id"].as_str().unwrap().to_owned();

    // RBAC rows: read with analysis.read, create/archive with experiment.write, members with
    // session.read too; another org's dataset is a 404.
    let detail = format!("/v1/research/datasets/{id}");
    let members = format!("{detail}/members");
    for (role, read, write) in [
        (Role::Owner, true, true),
        (Role::Researcher, true, true),
        (Role::Admin, true, false),
        (Role::Clinician, true, false),
        (Role::Reviewer, false, false),
        (Role::Viewer, false, false),
    ] {
        let (_, t) = c.token(role).await;
        let (s, _) = c.call(Method::GET, "/v1/research/datasets", None, &t).await;
        assert_eq!(s.is_success(), read, "{role:?} list {s}");
        let (s, _) = c.call(Method::GET, &detail, None, &t).await;
        assert_eq!(s.is_success(), read, "{role:?} detail {s}");
        let (s, _) = c.call(Method::GET, &members, None, &t).await;
        assert_eq!(s.is_success(), read, "{role:?} members {s}");
        let (s, _) = create(
            &c,
            json!({"name": format!("by-{role:?}"), "cohort_id": cohort}),
            &t,
        )
        .await;
        assert_eq!(s == StatusCode::CREATED, write, "{role:?} create {s}");
        if !write {
            assert_eq!(s, StatusCode::FORBIDDEN, "{role:?}");
        }
    }
    let (_, vendor) = c.token(Role::Superadmin).await;
    let (s, _) = c
        .call(Method::GET, "/v1/research/datasets", None, &vendor)
        .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    let (s, page) = c
        .call(
            Method::GET,
            &format!("/v1/research/datasets?org_id={}", c.org),
            None,
            &vendor,
        )
        .await;
    assert_eq!(s, StatusCode::OK, "{page}");
    assert!(!page["items"].as_array().unwrap().is_empty());
    let (s, _) = create(&c, json!({"name": "vendor", "cohort_id": cohort}), &vendor).await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    let (_, stranger) = c.token_in(Some(c.other), Role::Researcher).await;
    for uri in [&detail, &members] {
        let (s, _) = c.call(Method::GET, uri, None, &stranger).await;
        assert_eq!(s, StatusCode::NOT_FOUND, "{uri}");
    }
    let (s, _) = c
        .call(Method::POST, &format!("{detail}/archive"), None, &stranger)
        .await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    let (s, _) = c
        .call(
            Method::GET,
            &format!("/v1/research/datasets?org_id={}", c.org),
            None,
            &stranger,
        )
        .await;
    assert_eq!(s, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn runs_read_the_frozen_members() {
    let Some(c) = analysis_support::ctx_with(|_| {}).await else {
        return;
    };
    analysis_support::cohort(&c, None).await;
    let (_, researcher) = c.token(Role::Researcher).await;
    let (exp_id, _) = analysis_support::experiment(&c, &researcher).await;
    let cohort = create_cohort(&c, &researcher, "all", json!({})).await;
    let (s, d) = create(
        &c,
        json!({"name": "frozen", "cohort_id": cohort, "exclusion_rules": [
            {"kind": "explicit_list", "id": "m",
             "sessions": [{"session_id": "C-05", "reason": "duplicate"}]}]}),
        &researcher,
    )
    .await;
    assert_eq!(s, StatusCode::CREATED, "{d}");
    assert_eq!(d["member_count"], 29);
    let dataset = d["id"].as_str().unwrap().to_owned();
    // The cohort grows after the freeze; the dataset does not.
    analysis_support::add_session(&c, c.org, "C-99", 99, 30, None).await;

    let (s, b) = analysis_support::create_run(
        &c,
        json!({"kind": "cohort_rescore", "experiment_id": exp_id, "cohort_id": cohort,
               "dataset_id": dataset, "level": "score", "seed": 1}),
        &researcher,
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{b}");

    let (s, run) = analysis_support::create_run(
        &c,
        json!({"kind": "cohort_rescore", "experiment_id": exp_id, "dataset_id": dataset,
               "level": "score", "seed": 1}),
        &researcher,
    )
    .await;
    assert_eq!(s, StatusCode::ACCEPTED, "{run}");
    assert_eq!(run["dataset_id"], dataset.as_str());
    assert_eq!(run["dataset_hash"], d["content_hash"]);
    assert_eq!(run["cohort_id"], cohort.as_str());
    assert_eq!(run["definition"]["dataset"]["id"], dataset.as_str());
    assert_eq!(c.run_jobs().await, 1);
    let id = run["id"].as_str().unwrap();
    let done = analysis_support::get_run(&c, id, &researcher).await;
    assert_eq!(done["status"], "succeeded", "{done}");
    assert_eq!(done["sessions_n"], 29);

    // A member erased after the freeze is `session_gone` in the next run.
    let participant = c.participant(c.org, "TOK-C-07").await;
    let (_, admin) = c.token(Role::Admin).await;
    let (s, b) = c
        .call(
            Method::POST,
            &format!("/v1/orgs/{}/participants/{participant}/erasure", c.org),
            None,
            &admin,
        )
        .await;
    assert!(s.is_success(), "{s} {b}");
    c.run_jobs().await;
    let (s, rerun) = c
        .call(
            Method::POST,
            &format!("/v1/analysis-runs/{id}/rerun"),
            None,
            &researcher,
        )
        .await;
    assert_eq!(s, StatusCode::ACCEPTED, "{rerun}");
    assert_eq!(rerun["dataset_id"], dataset.as_str());
    c.run_jobs().await;
    let again = analysis_support::get_run(&c, rerun["id"].as_str().unwrap(), &researcher).await;
    assert_eq!(again["status"], "succeeded", "{again}");
    assert_eq!(again["sessions_n"], 28);
    assert_eq!(again["excluded"]["session_gone"], 1, "{again}");

    // An archived dataset starts no run.
    let (s, _) = c
        .call(
            Method::POST,
            &format!("/v1/research/datasets/{dataset}/archive"),
            None,
            &researcher,
        )
        .await;
    assert_eq!(s, StatusCode::OK);
    let (s, b) = analysis_support::create_run(
        &c,
        json!({"kind": "cohort_rescore", "experiment_id": exp_id, "dataset_id": dataset,
               "level": "score"}),
        &researcher,
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{b}");
}
