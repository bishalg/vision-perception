//! owner: WP-X.2
//!
//! The research frame log, server side (RFL-3..6; acceptance 4 and 5): the ingest ack offers
//! the sealed upload in proxy mode only and `age_months` is stored; `PUT
//! .../blobs/research_frames` needs a digest, a matching header, research consent and a live
//! participant, and stores a sealed object (not JSON); the session detail and raw read show
//! it; `GET .../research-frames` is for owner and researcher only, audited, `no-store`; the
//! retention sweep purges it on the org's research horizon (`410 frames-purged`) and an
//! erasure deletes it. Needs `DATABASE_URL`; skipped otherwise.

mod common;
mod ingest_support;
mod research_support;

use std::sync::Arc;

use asd_backend::blob::MemoryBlobStore;
use asd_backend::config::BlobMode;
use asd_contract::api_v1::Role;
use http::{Method, StatusCode};
use ingest_support::{golden_with_id, post_session, problem_slug, send_raw};
use research_support::*;
use serde_json::{json, Value};

/// The iOS golden session with research consent and an age.
fn consented_session(id: &str, consented: bool) -> Value {
    let mut v = golden_with_id(id);
    v["rawPayload"]["normCollection"] = json!({"consented": consented});
    v["rawPayload"]["ageMonths"] = json!(30);
    v
}

async fn ingest(c: &Ctx, device: &str, body: &Value) -> Value {
    let (s, _, b) = common::send(c.app.clone(), post_session(Some(device), body)).await;
    assert_eq!(s, StatusCode::CREATED, "{b}");
    b
}

async fn upload(c: &Ctx, device: &str, session: &str) {
    let body = research_log(session);
    let d = digest(&body);
    let (s, _, b) = send_raw(c.app.clone(), put_research(device, session, body, Some(&d))).await;
    assert_eq!(s, StatusCode::NO_CONTENT, "{}", String::from_utf8_lossy(&b));
}

#[tokio::test]
async fn upload_rules_ack_and_detail() {
    let Some(c) = Ctx::new().await else { return };
    let (_, device) = common::auth::seed_device(&c.db.pool, c.org).await;
    let (_, owner) = c.token(Role::Owner).await;

    // The ack of a consented session offers the sealed upload (proxy mode); a session
    // without research consent gets none.
    let ack = ingest(&c, &device, &consented_session("S-RES", true)).await;
    assert_eq!(
        ack["research_frames_upload_url"],
        "/v1/sessions/S-RES/blobs/research_frames"
    );
    assert_eq!(ack["frames_upload_url"], "/v1/sessions/S-RES/blobs/frames");
    let ack = ingest(&c, &device, &consented_session("S-NOC", false)).await;
    assert!(ack.get("research_frames_upload_url").is_none(), "{ack}");
    // A replay recomputes it like the other URLs.
    let (s, _, replay) = common::send(
        c.app.clone(),
        post_session(Some(&device), &consented_session("S-RES", true)),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{replay}");
    assert_eq!(replay["duplicate"], true);
    assert_eq!(
        replay["research_frames_upload_url"],
        "/v1/sessions/S-RES/blobs/research_frames"
    );
    // COH-3: ingest stores the age as a column.
    let age: Option<i32> = sqlx::query_scalar("SELECT age_months FROM sessions WHERE id = 'S-RES'")
        .fetch_one(&c.db.pool)
        .await
        .unwrap();
    assert_eq!(age, Some(30));

    let body = research_log("S-RES");
    let good = digest(&body);
    // No Content-Digest.
    let (s, _, b) = send_raw(
        c.app.clone(),
        put_research(&device, "S-RES", body.clone(), None),
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    assert_eq!(
        problem_slug(&serde_json::from_slice(&b).unwrap()),
        "digest-required"
    );
    // A wrong digest.
    let (s, _, b) = send_raw(
        c.app.clone(),
        put_research(&device, "S-RES", body.clone(), Some(&digest(b"other"))),
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    assert_eq!(
        problem_slug(&serde_json::from_slice(&b).unwrap()),
        "digest-mismatch"
    );
    // A header naming another session.
    let other = research_log("S-ELSE");
    let (s, _, b) = send_raw(
        c.app.clone(),
        put_research(&device, "S-RES", other.clone(), Some(&digest(&other))),
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    assert_eq!(
        problem_slug(&serde_json::from_slice(&b).unwrap()),
        "validation-failed"
    );
    // Not a raw log at all.
    let junk = b"{\"format\":\"something-else\"}\n".to_vec();
    let (s, _, _) = send_raw(
        c.app.clone(),
        put_research(&device, "S-RES", junk.clone(), Some(&digest(&junk))),
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    // No research consent.
    let noc = research_log("S-NOC");
    let (s, _, b) = send_raw(
        c.app.clone(),
        put_research(&device, "S-NOC", noc.clone(), Some(&digest(&noc))),
    )
    .await;
    assert_eq!(s, StatusCode::CONFLICT);
    assert_eq!(
        problem_slug(&serde_json::from_slice(&b).unwrap()),
        "research-consent-required"
    );
    // Another org's device: 404.
    let (_, other_device) = common::auth::seed_device(&c.db.pool, c.other).await;
    let (s, _, _) = send_raw(
        c.app.clone(),
        put_research(&other_device, "S-RES", body.clone(), Some(&good)),
    )
    .await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    assert!(c.audit_rows("session.frames_upload").await.is_empty());

    // Valid: 204, a sealed object that is not JSON, the columns and one audit row.
    let (s, _, b) = send_raw(
        c.app.clone(),
        put_research(&device, "S-RES", body.clone(), Some(&good)),
    )
    .await;
    assert_eq!(s, StatusCode::NO_CONTENT, "{}", String::from_utf8_lossy(&b));
    let key = format!(
        "sessions/{}/{}/S-RES/research.jsonl.sealed",
        c.org,
        consented_session("S-RES", true)["participantToken"]
            .as_str()
            .unwrap()
    );
    let stored = c
        .state
        .blobs
        .get(&key)
        .await
        .unwrap()
        .expect("sealed object");
    assert_eq!(stored.content_type, "application/octet-stream");
    assert!(serde_json::from_slice::<Value>(&stored.bytes).is_err());
    assert!(!stored
        .bytes
        .windows(b"asd-raw-tracking".len())
        .any(|w| w == b"asd-raw-tracking"));
    let rows = c.audit_rows("session.frames_upload").await;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["actor_type"], "device");
    assert_eq!(rows[0]["after"]["bytes"], body.len());
    assert_eq!(rows[0]["after"]["sha256"].as_str().unwrap().len(), 64);

    // RFL-6: the detail, the raw read and the list filter.
    let (s, d) = c
        .call(Method::GET, "/v1/sessions/S-RES", None, &owner)
        .await;
    assert_eq!(s, StatusCode::OK, "{d}");
    assert_eq!(
        d["research_frames"],
        json!({"available": true, "purged": false, "bytes": body.len()})
    );
    let (_, d) = c
        .call(Method::GET, "/v1/sessions/S-NOC", None, &owner)
        .await;
    assert_eq!(
        d["research_frames"],
        json!({"available": false, "purged": false, "bytes": null})
    );
    let (_, raw) = c
        .call(Method::GET, "/v1/sessions/S-RES/raw", None, &owner)
        .await;
    assert_eq!(raw["research_frames"]["available"], true);
    let list = |q: &'static str| {
        let c = &c;
        let owner = owner.clone();
        async move {
            let (s, b) = c
                .call(
                    Method::GET,
                    &format!("/v1/orgs/{}/sessions?{q}", c.org),
                    None,
                    &owner,
                )
                .await;
            assert_eq!(s, StatusCode::OK, "{b}");
            b["items"]
                .as_array()
                .unwrap()
                .iter()
                .map(|i| i["id"].as_str().unwrap().to_owned())
                .collect::<Vec<_>>()
        }
    };
    assert_eq!(list("frames=available").await, ["S-RES"]);
    assert_eq!(list("frames=none").await, ["S-NOC"]);
    let (s, _) = c
        .call(
            Method::GET,
            &format!("/v1/orgs/{}/sessions?frames=maybe", c.org),
            None,
            &owner,
        )
        .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);

    // A re-PUT replaces the log (same key, one more audit row).
    upload(&c, &device, "S-RES").await;
    assert_eq!(c.audit_rows("session.frames_upload").await.len(), 2);

    // A withdrawn participant's log is refused like any upload.
    let token = consented_session("S-RES", true)["participantToken"]
        .as_str()
        .unwrap()
        .to_owned();
    c.withdraw(c.org, &token).await;
    let (s, _, b) = send_raw(
        c.app.clone(),
        put_research(&device, "S-RES", body, Some(&good)),
    )
    .await;
    assert_eq!(s, StatusCode::CONFLICT);
    assert_eq!(
        problem_slug(&serde_json::from_slice(&b).unwrap()),
        "participant-withdrawn"
    );
    c.db.cleanup().await;
}

#[tokio::test]
async fn presigned_mode_offers_no_research_upload() {
    let Some(db) = common::TestDb::migrated().await else {
        return;
    };
    let mut config = ingest_support::config(false);
    config.blob_mode = BlobMode::Presigned;
    let state =
        ingest_support::state_with(db.pool.clone(), Arc::new(MemoryBlobStore::new()), config);
    let c = Ctx::with_state(db, state).await;
    let (_, device) = common::auth::seed_device(&c.db.pool, c.org).await;
    let ack = ingest(&c, &device, &consented_session("S-PRE", true)).await;
    assert!(ack["frames_upload_url"].is_string(), "{ack}");
    assert!(ack.get("research_frames_upload_url").is_none(), "{ack}");
    let body = research_log("S-PRE");
    let d = digest(&body);
    let (s, _, _) = send_raw(
        c.app.clone(),
        put_research(&device, "S-PRE", body, Some(&d)),
    )
    .await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    c.db.cleanup().await;
}

#[tokio::test]
async fn read_is_for_owner_and_researcher_audited_and_purged_by_retention() {
    let Some(c) = Ctx::new().await else { return };
    let (_, device) = common::auth::seed_device(&c.db.pool, c.org).await;
    ingest(&c, &device, &consented_session("S-R1", true)).await;
    ingest(&c, &device, &consented_session("S-R2", true)).await;
    upload(&c, &device, "S-R1").await;
    upload(&c, &device, "S-R2").await;
    let (_, researcher) = c.token(Role::Researcher).await;
    let (_, owner) = c.token(Role::Owner).await;
    let (_, clinician) = c.token(Role::Clinician).await;
    let (_, admin) = c.token(Role::Admin).await;
    let (_, root) = c.token(Role::Superadmin).await;
    let (_, stranger) = c.token_in(Some(c.other), Role::Researcher).await;

    let get = |uri: &'static str, bearer: String| {
        let c = &c;
        async move {
            let req = common::auth::empty(Method::GET, uri, Some(&bearer));
            send_raw(c.app.clone(), req).await
        }
    };
    let (s, h, bytes) = get("/v1/sessions/S-R1/research-frames", researcher.clone()).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(bytes, research_log("S-R1"));
    assert_eq!(common::content_type(&h), "application/x-ndjson");
    assert!(h["cache-control"].to_str().unwrap().contains("no-store"));
    let (s, _, _) = get("/v1/sessions/S-R1/research-frames", owner.clone()).await;
    assert_eq!(s, StatusCode::OK);
    let reads = c.audit_rows("session.frames_read").await;
    assert_eq!(reads.len(), 2);
    assert_eq!(reads[0]["target_id"], "S-R1");
    assert_eq!(reads[0]["org_id"], c.org.to_string());
    // GOV-1: never the vendor; not the clinician or admin; another org's user gets 404.
    for (bearer, want) in [
        (root, StatusCode::FORBIDDEN),
        (clinician, StatusCode::FORBIDDEN),
        (admin.clone(), StatusCode::FORBIDDEN),
        (stranger, StatusCode::NOT_FOUND),
    ] {
        let (s, _, _) = get("/v1/sessions/S-R1/research-frames", bearer).await;
        assert_eq!(s, want);
    }
    assert_eq!(c.audit_rows("session.frames_read").await.len(), 2);
    // Never uploaded: 404 frames-not-available.
    ingest(&c, &device, &consented_session("S-R3", true)).await;
    let (s, _, b) = get("/v1/sessions/S-R3/research-frames", researcher.clone()).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    assert_eq!(
        problem_slug(&serde_json::from_slice(&b).unwrap()),
        "frames-not-available"
    );

    // RFL-5: a one-month research horizon (the org route validates and audits it); a log
    // received two months ago is purged, the recent one stays; the session row stays.
    let (s, org) = c
        .call(
            Method::PATCH,
            &format!("/v1/orgs/{}", c.org),
            Some(json!({"research_frames_retention_months": 0})),
            &owner,
        )
        .await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{org}");
    let (s, org) = c
        .call(
            Method::PATCH,
            &format!("/v1/orgs/{}", c.org),
            Some(json!({"research_frames_retention_months": 1})),
            &owner,
        )
        .await;
    assert_eq!(s, StatusCode::OK, "{org}");
    assert_eq!(org["research_frames_retention_months"], 1);
    assert_eq!(org["retention_months"], 24);
    let (_, got) = c
        .call(Method::GET, &format!("/v1/orgs/{}", c.org), None, &owner)
        .await;
    assert_eq!(got["research_frames_retention_months"], 1);
    let update = c.audit_rows("org.update").await;
    assert_eq!(
        update.last().unwrap()["after"]["research_frames_retention_months"],
        1
    );
    sqlx::query("UPDATE sessions SET received_at = now() - interval '2 months' WHERE id = 'S-R1'")
        .execute(&c.db.pool)
        .await
        .unwrap();
    let key: String =
        sqlx::query_scalar("SELECT research_frames_blob_key FROM sessions WHERE id = 'S-R1'")
            .fetch_one(&c.db.pool)
            .await
            .unwrap();
    let uri = format!("/v1/orgs/{}/retention-sweep", c.org);
    let (s, job) = c
        .call(Method::POST, &uri, Some(json!({"dry_run": true})), &admin)
        .await;
    assert_eq!(s, StatusCode::ACCEPTED, "{job}");
    c.run_jobs().await;
    let (_, done) = c
        .call(
            Method::GET,
            &format!("/v1/jobs/{}", job["job_id"].as_str().unwrap()),
            None,
            &admin,
        )
        .await;
    assert_eq!(
        done["result"]["orgs"][0]["research_frames_purged"], 1,
        "{done}"
    );
    assert!(c.state.blobs.get(&key).await.unwrap().is_some(), "dry run");
    let (s, job) = c
        .call(Method::POST, &uri, Some(json!({"dry_run": false})), &admin)
        .await;
    assert_eq!(s, StatusCode::ACCEPTED, "{job}");
    c.run_jobs().await;
    let (_, done) = c
        .call(
            Method::GET,
            &format!("/v1/jobs/{}", job["job_id"].as_str().unwrap()),
            None,
            &admin,
        )
        .await;
    assert_eq!(done["status"], "succeeded", "{done}");
    assert_eq!(
        done["result"]["orgs"][0]["research_frames_purged"], 1,
        "{done}"
    );
    // The session's own frames horizon (24 months) is untouched.
    assert_eq!(done["result"]["orgs"][0]["frames_purged"], 0, "{done}");
    assert!(c.state.blobs.get(&key).await.unwrap().is_none());
    let (s, _, b) = get("/v1/sessions/S-R1/research-frames", researcher.clone()).await;
    assert_eq!(s, StatusCode::GONE);
    assert_eq!(
        problem_slug(&serde_json::from_slice(&b).unwrap()),
        "frames-purged"
    );
    let (_, d) = c.call(Method::GET, "/v1/sessions/S-R1", None, &owner).await;
    assert_eq!(
        d["research_frames"],
        json!({"available": false, "purged": true, "bytes": null})
    );
    let (s, _, _) = get("/v1/sessions/S-R2/research-frames", researcher.clone()).await;
    assert_eq!(s, StatusCode::OK, "inside the horizon");
    let sweep = c.audit_rows("retention.sweep").await;
    assert_eq!(sweep.last().unwrap()["after"]["research_frames_purged"], 1);

    // `null` returns the org to `retention_months`.
    let (s, org) = c
        .call(
            Method::PATCH,
            &format!("/v1/orgs/{}", c.org),
            Some(json!({"research_frames_retention_months": null})),
            &owner,
        )
        .await;
    assert_eq!(s, StatusCode::OK, "{org}");
    assert!(
        org.get("research_frames_retention_months").is_none(),
        "{org}"
    );
    c.db.cleanup().await;
}

#[tokio::test]
async fn erasure_deletes_the_sealed_log() {
    let Some(c) = Ctx::new().await else { return };
    let (_, device) = common::auth::seed_device(&c.db.pool, c.org).await;
    ingest(&c, &device, &consented_session("S-E1", true)).await;
    upload(&c, &device, "S-E1").await;
    let key: String =
        sqlx::query_scalar("SELECT research_frames_blob_key FROM sessions WHERE id = 'S-E1'")
            .fetch_one(&c.db.pool)
            .await
            .unwrap();
    assert!(c.state.blobs.get(&key).await.unwrap().is_some());
    let participant: uuid::Uuid =
        sqlx::query_scalar("SELECT participant_id FROM sessions WHERE id = 'S-E1'")
            .fetch_one(&c.db.pool)
            .await
            .unwrap();
    let (_, admin) = c.token(Role::Admin).await;
    let (s, b) = c
        .call(
            Method::POST,
            &format!("/v1/orgs/{}/participants/{participant}/erasure", c.org),
            None,
            &admin,
        )
        .await;
    assert_eq!(s, StatusCode::ACCEPTED, "{b}");
    c.run_jobs().await;
    assert!(c.state.blobs.get(&key).await.unwrap().is_none());
    let left: i64 = sqlx::query_scalar("SELECT count(*) FROM sessions WHERE id = 'S-E1'")
        .fetch_one(&c.db.pool)
        .await
        .unwrap();
    assert_eq!(left, 0);
    c.db.cleanup().await;
}

/// A held participant (a clinical note documents them) keeps the session row, but the
/// research log is research data and goes; the row says so.
#[tokio::test]
async fn erasure_of_a_held_participant_still_deletes_the_log() {
    let Some(c) = Ctx::new().await else { return };
    let (_, device) = common::auth::seed_device(&c.db.pool, c.org).await;
    ingest(&c, &device, &consented_session("S-H1", true)).await;
    upload(&c, &device, "S-H1").await;
    let key: String =
        sqlx::query_scalar("SELECT research_frames_blob_key FROM sessions WHERE id = 'S-H1'")
            .fetch_one(&c.db.pool)
            .await
            .unwrap();
    let participant: uuid::Uuid =
        sqlx::query_scalar("SELECT participant_id FROM sessions WHERE id = 'S-H1'")
            .fetch_one(&c.db.pool)
            .await
            .unwrap();
    let (author, _) = c.token(Role::Clinician).await;
    sqlx::query(
        "INSERT INTO clinical_notes (org_id, participant_id, session_id, author_id, kind,
                                     body_ct, body_key_id)
         SELECT $1, $2, 'S-H1', $3, 'general', '\\x00'::bytea, gen_random_uuid()",
    )
    .bind(c.org)
    .bind(participant)
    .bind(author)
    .execute(&c.db.pool)
    .await
    .expect("a clinical note (hold)");
    let (_, admin) = c.token(Role::Admin).await;
    let (s, b) = c
        .call(
            Method::POST,
            &format!("/v1/orgs/{}/participants/{participant}/erasure", c.org),
            None,
            &admin,
        )
        .await;
    assert_eq!(s, StatusCode::ACCEPTED, "{b}");
    c.run_jobs().await;
    let status: String =
        sqlx::query_scalar("SELECT status FROM erasure_requests WHERE participant_id = $1")
            .bind(participant)
            .fetch_one(&c.db.pool)
            .await
            .unwrap();
    assert_eq!(status, "held");
    assert!(c.state.blobs.get(&key).await.unwrap().is_none());
    let (k, purged): (Option<String>, bool) = sqlx::query_as(
        "SELECT research_frames_blob_key, research_frames_purged_at IS NOT NULL
         FROM sessions WHERE id = 'S-H1'",
    )
    .fetch_one(&c.db.pool)
    .await
    .unwrap();
    assert_eq!((k, purged), (None, true));
    c.db.cleanup().await;
}
