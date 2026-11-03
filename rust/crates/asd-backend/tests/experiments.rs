//! owner: WP-X.2
//!
//! Experiments (EXP-1, EXP-2, GOV-1, GOV-2; acceptance 13 first half): create from the
//! published profile (or the defaults), from another experiment or a named document; name
//! conflicts `409 experiment-name-exists`; `PATCH` with `If-Match` (`412 stale-version`) and
//! the parameter registry (`400 config-invalid` with `field_problems`); archive; promote
//! (`409 experiment-no-run` without a run, then a draft document with the evidence notes and
//! the researcher as author, `409 experiment-promoted` the second time); RBAC and audits.
//! Needs `DATABASE_URL`; skipped otherwise.

mod common;
mod research_support;

use asd_contract::api_v1::Role;
use asd_contract::config::ScoringProfile;
use http::{Method, StatusCode};
use research_support::*;
use serde_json::{json, Value};
use uuid::Uuid;

fn slug(b: &Value) -> &str {
    common::auth::slug(b)
}

fn profile() -> Value {
    serde_json::to_value(ScoringProfile::default()).unwrap()
}

#[tokio::test]
async fn create_list_get_and_rbac() {
    let Some(c) = Ctx::new().await else { return };
    let (researcher_id, researcher) = c.token(Role::Researcher).await;
    let (_, owner) = c.token(Role::Owner).await;
    let (_, clinician) = c.token(Role::Clinician).await;
    let (_, admin) = c.token(Role::Admin).await;
    let (_, root) = c.token(Role::Superadmin).await;
    let (_, stranger) = c.token_in(Some(c.other), Role::Researcher).await;
    let uri = format!("/v1/orgs/{}/experiments", c.org);

    // Nothing published: the default profile, base version `default`.
    let (s, e) = c
        .call(
            Method::POST,
            &uri,
            Some(json!({"name": "  Tighter I-VT ", "description": "first"})),
            &researcher,
        )
        .await;
    assert_eq!(s, StatusCode::CREATED, "{e}");
    assert_eq!(e["name"], "Tighter I-VT");
    assert_eq!(e["status"], "draft");
    assert_eq!(e["base_profile_version"], "default");
    assert_eq!(e["row_version"], 1);
    assert_eq!(e["created_by"], researcher_id.to_string());
    assert_eq!(e["profile"], profile());
    assert_eq!(
        e["profile_hash"],
        jcs_hash(&profile())
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    );
    let id = e["id"].as_str().unwrap().to_owned();

    // Same name in the org: 409; in another org: fine.
    let (s, b) = c
        .call(
            Method::POST,
            &uri,
            Some(json!({"name": "Tighter I-VT"})),
            &owner,
        )
        .await;
    assert_eq!(s, StatusCode::CONFLICT, "{b}");
    assert_eq!(slug(&b), "experiment-name-exists");
    let (s, b) = c
        .call(
            Method::POST,
            &format!("/v1/orgs/{}/experiments", c.other),
            Some(json!({"name": "Tighter I-VT"})),
            &stranger,
        )
        .await;
    assert_eq!(s, StatusCode::CREATED, "{b}");

    // Bad names, two sources at once, unknown members.
    for body in [
        json!({"name": ""}),
        json!({"name": "x".repeat(81)}),
        json!({"name": "a", "description": "d".repeat(501)}),
        json!({"name": "a", "from_version": "1", "from_experiment_id": id}),
        json!({"name": "a", "colour": "red"}),
    ] {
        let (s, b) = c
            .call(Method::POST, &uri, Some(body.clone()), &researcher)
            .await;
        assert_eq!(s, StatusCode::BAD_REQUEST, "{body} -> {b}");
    }
    // From another experiment (same org only) and from a named document.
    let (s, b) = c
        .call(
            Method::POST,
            &uri,
            Some(json!({"name": "Copy", "from_experiment_id": id})),
            &researcher,
        )
        .await;
    assert_eq!(s, StatusCode::CREATED, "{b}");
    assert_eq!(b["profile_hash"], e["profile_hash"]);
    let (s, _) = c
        .call(
            Method::POST,
            &format!("/v1/orgs/{}/experiments", c.other),
            Some(json!({"name": "Steal", "from_experiment_id": id})),
            &stranger,
        )
        .await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    let (s, _) = c
        .call(
            Method::POST,
            &uri,
            Some(json!({"name": "From doc", "from_version": "nope"})),
            &researcher,
        )
        .await;
    assert_eq!(s, StatusCode::NOT_FOUND);

    // GOV-1: write is owner and researcher; not admin, clinician or the vendor.
    for bearer in [&clinician, &admin, &root] {
        let (s, _) = c
            .call(Method::POST, &uri, Some(json!({"name": "No"})), bearer)
            .await;
        assert_eq!(s, StatusCode::FORBIDDEN);
    }
    // Reads: AnalysisRead (clinician, admin and the vendor included); other orgs 404.
    let (s, page) = c.call(Method::GET, &uri, None, &clinician).await;
    assert_eq!(s, StatusCode::OK, "{page}");
    assert_eq!(page["items"].as_array().unwrap().len(), 2);
    let (s, one) = c
        .call(Method::GET, &format!("{uri}?limit=1"), None, &admin)
        .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(one["items"].as_array().unwrap().len(), 1);
    let next = one["next_cursor"].as_str().unwrap();
    let (_, two) = c
        .call(
            Method::GET,
            &format!("{uri}?limit=1&cursor={next}"),
            None,
            &admin,
        )
        .await;
    assert_eq!(two["items"].as_array().unwrap().len(), 1);
    assert_ne!(two["items"][0]["id"], one["items"][0]["id"]);
    let (s, b) = c
        .call(Method::GET, &format!("/v1/experiments/{id}"), None, &root)
        .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    let (s, _) = c
        .call(
            Method::GET,
            &format!("/v1/experiments/{id}"),
            None,
            &stranger,
        )
        .await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    let (s, _) = c
        .call(Method::GET, &format!("{uri}?status=live"), None, &admin)
        .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);

    let rows = c.audit_rows("experiment.create").await;
    assert_eq!(rows.len(), 3, "two here, one in the other org");
    assert_eq!(rows[0]["target_id"], id);
    assert_eq!(rows[0]["after"]["profile_hash"], e["profile_hash"]);
    c.db.cleanup().await;
}

#[tokio::test]
async fn a_published_profile_is_the_starting_point() {
    let Some(c) = Ctx::new().await else { return };
    let (_, researcher) = c.token(Role::Researcher).await;
    let mut p = profile();
    p["flag"]["greenFloor"] = json!(0.7);
    p["schemaVersion"] = json!("1.1.0");
    sqlx::query(
        "INSERT INTO config_documents (kind, version, status, body, published_at)
         VALUES ('scoring_profile', 'pub-1', 'published', $1, now())",
    )
    .bind(&p)
    .execute(&c.db.pool)
    .await
    .unwrap();
    let (s, e) = c
        .call(
            Method::POST,
            &format!("/v1/orgs/{}/experiments", c.org),
            Some(json!({"name": "From published"})),
            &researcher,
        )
        .await;
    assert_eq!(s, StatusCode::CREATED, "{e}");
    assert_eq!(e["base_profile_version"], "pub-1");
    assert_eq!(e["profile"]["flag"]["greenFloor"], 0.7);
    // Stored whole, at 1.2.0.
    assert_eq!(e["profile"]["schemaVersion"], "1.2.0");
    assert_eq!(e["profile"]["metrics"], profile()["metrics"]);
    c.db.cleanup().await;
}

#[tokio::test]
async fn patch_needs_if_match_and_a_registry_valid_profile() {
    let Some(c) = Ctx::new().await else { return };
    let (_, researcher) = c.token(Role::Researcher).await;
    let (_, e) = c
        .call(
            Method::POST,
            &format!("/v1/orgs/{}/experiments", c.org),
            Some(json!({"name": "E"})),
            &researcher,
        )
        .await;
    let uri = format!("/v1/experiments/{}", e["id"].as_str().unwrap());
    let mut p = profile();
    p["metrics"]["ivtVelocityUnitsPerSec"] = json!(2.0);

    let (s, b) = c
        .patch_if(&uri, json!({"profile": p}), None, &researcher)
        .await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{b}");
    let (s, b) = c
        .patch_if(&uri, json!({"profile": p}), Some("\"7\""), &researcher)
        .await;
    assert_eq!(s, StatusCode::PRECONDITION_FAILED, "{b}");
    assert_eq!(slug(&b), "stale-version");

    // PRM-3: range, unknown key and a cross-field rule, each with its path.
    let mut bad = p.clone();
    bad["flag"]["greenFloor"] = json!(1.5);
    bad["metrics"]["ivtVelocityUnitsPerSec"] = json!(100.0);
    bad["aoi"]["colour"] = json!("red");
    let (s, b) = c
        .patch_if(&uri, json!({"profile": bad}), Some("\"1\""), &researcher)
        .await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{b}");
    assert_eq!(slug(&b), "config-invalid");
    let paths: Vec<&str> = b["field_problems"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["path"].as_str().unwrap())
        .collect();
    for want in [
        "flag.greenFloor",
        "metrics.ivtVelocityUnitsPerSec",
        "aoi.colour",
    ] {
        assert!(paths.contains(&want), "{want} in {paths:?}");
    }
    assert!(b["field_problems"][0]["message"].is_string());
    // A baselines version that does not exist.
    let (s, _) = c
        .patch_if(
            &uri,
            json!({"baselines_version": "nope"}),
            Some("\"1\""),
            &researcher,
        )
        .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    assert!(c.audit_rows("experiment.update").await.is_empty());

    let (s, b) = c
        .patch_if(
            &uri,
            json!({"profile": p, "description": "slower saccades"}),
            Some("W/\"1\""),
            &researcher,
        )
        .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    assert_eq!(b["row_version"], 2);
    assert_eq!(b["profile"]["metrics"]["ivtVelocityUnitsPerSec"], 2.0);
    assert_eq!(b["description"], "slower saccades");
    assert_ne!(b["profile_hash"], e["profile_hash"]);
    let rows = c.audit_rows("experiment.update").await;
    assert_eq!(rows.len(), 1);
    assert_eq!(
        rows[0]["after"]["changed_params"],
        json!(["metrics.ivtVelocityUnitsPerSec"])
    );
    // Values never reach the audit row.
    assert!(!rows[0].to_string().contains("slower saccades"));
    // The old version is stale now.
    let (s, _) = c
        .patch_if(&uri, json!({"name": "E2"}), Some("\"1\""), &researcher)
        .await;
    assert_eq!(s, StatusCode::PRECONDITION_FAILED);

    // Archive ends the draft: no more edits, no second archive.
    let (s, b) = c
        .call(Method::POST, &format!("{uri}/archive"), None, &researcher)
        .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    assert_eq!(b["status"], "archived");
    let (s, _) = c
        .patch_if(&uri, json!({"name": "E3"}), Some("\"3\""), &researcher)
        .await;
    assert_eq!(s, StatusCode::CONFLICT);
    let (s, _) = c
        .call(Method::POST, &format!("{uri}/archive"), None, &researcher)
        .await;
    assert_eq!(s, StatusCode::CONFLICT);
    assert_eq!(c.audit_rows("experiment.archive").await.len(), 1);
    c.db.cleanup().await;
}

#[tokio::test]
async fn promote_needs_a_run_and_files_a_researcher_draft() {
    let Some(c) = Ctx::new().await else { return };
    let (researcher_id, researcher) = c.token(Role::Researcher).await;
    let (_, admin) = c.token(Role::Admin).await;
    let (_, e) = c
        .call(
            Method::POST,
            &format!("/v1/orgs/{}/experiments", c.org),
            Some(json!({"name": "To promote"})),
            &researcher,
        )
        .await;
    let id: Uuid = e["id"].as_str().unwrap().parse().unwrap();
    let uri = format!("/v1/experiments/{id}/promote");
    let body = json!({"kind": "scoring_profile", "version": "x-1", "notes": "for review"});

    // GOV-1: admin may not promote.
    let (s, _) = c.call(Method::POST, &uri, Some(body.clone()), &admin).await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    let (s, b) = c
        .call(Method::POST, &uri, Some(body.clone()), &researcher)
        .await;
    assert_eq!(s, StatusCode::CONFLICT, "{b}");
    assert_eq!(slug(&b), "experiment-no-run");

    let run = c
        .finished_run(c.org, Some(id), jcs_hash(&e["profile"]), "b")
        .await;
    let (s, _) = c
        .call(
            Method::POST,
            &uri,
            Some(json!({"kind": "stimulus_catalog", "version": "x-1"})),
            &researcher,
        )
        .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    let (s, doc) = c
        .call(Method::POST, &uri, Some(body.clone()), &researcher)
        .await;
    assert_eq!(s, StatusCode::CREATED, "{doc}");
    assert_eq!(doc["kind"], "scoring_profile");
    assert_eq!(doc["version"], "x-1");
    assert_eq!(doc["status"], "draft");
    assert_eq!(doc["created_by"], researcher_id.to_string());
    assert_eq!(
        doc["notes"],
        format!("experiment:{id} run:{run}\nfor review")
    );
    assert_eq!(doc["body"], e["profile"]);

    let (_, after) = c
        .call(
            Method::GET,
            &format!("/v1/experiments/{id}"),
            None,
            &researcher,
        )
        .await;
    assert_eq!(after["status"], "promoted");
    assert_eq!(after["promoted_config_version"], "x-1");
    assert_eq!(after["last_run_id"], run.to_string());
    let (s, b) = c.call(Method::POST, &uri, Some(body), &researcher).await;
    assert_eq!(s, StatusCode::CONFLICT, "{b}");
    assert_eq!(slug(&b), "experiment-promoted");

    let promote = c.audit_rows("experiment.promote").await;
    assert_eq!(promote.len(), 1);
    assert_eq!(promote[0]["actor_id"], researcher_id.to_string());
    assert_eq!(promote[0]["after"]["version"], "x-1");
    let create = c.audit_rows("config.document.create").await;
    assert_eq!(create.len(), 1);
    assert_eq!(create[0]["actor_id"], researcher_id.to_string());

    // The researcher cannot approve their draft (ConfigGlobalManage).
    let (s, _) = c
        .call(
            Method::POST,
            "/v1/config/scoring_profile/x-1/approve",
            None,
            &researcher,
        )
        .await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    c.db.cleanup().await;
}
