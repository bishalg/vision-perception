//! owner: WP-I.13
//!
//! Share links (SHR-8, SHR-9, HIP-8, HIP-9; acceptance 7 and 10 backend parts): the
//! passcode is returned once and stored as Argon2id, never listed, logged or audited;
//! wrong passcodes are `401` and audited with the client IP; the right one yields a
//! five-minute download token; downloads are counted atomically under concurrency;
//! revoked and unknown links are `404`, expired ones `410`; the open route is rate
//! limited per link and per IP; public responses are `no-store` and `noindex`; erasure
//! of the participant expires the package and revokes its links (SHR-11, WP-I.13b), also
//! when no included session names the participant. Needs `DATABASE_URL`.

mod common;
mod export_support;
mod participant_support;

use std::io::Write;
use std::sync::{Arc, Mutex};

use asd_backend::telemetry::RedactingJsonLayer;
use asd_contract::api_v1::Role;
use asd_contract::emr::SHARE_PASSCODE_ALPHABET;
use axum::body::Body;
use common::auth::from_ip;
use http::{HeaderMap, Method, Request, StatusCode};
use http_body_util::BodyExt;
use participant_support::*;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tower::ServiceExt;
use tracing_subscriber::fmt::MakeWriter;
use tracing_subscriber::layer::SubscriberExt;
use uuid::Uuid;

/// The tests of this binary run one at a time: the log test below must not share the
/// process with requests whose callsites register while its subscriber is installed only
/// on its own thread (their interest would be cached as "never" and nothing would be
/// captured).
static SERIAL: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// A ready identified case export and a clinician bearer.
async fn setup() -> Option<(Ctx, String, Value)> {
    let c = Ctx::new().await?;
    let (_, clin) = c.token(Role::Clinician).await;
    let f = c.case_fixture(&clin).await;
    let export = c.ready_case(&clin, f.participant).await;
    Some((c, clin, export))
}

async fn create_link(
    c: &Ctx,
    bearer: &str,
    export: &Value,
    body: Value,
) -> (StatusCode, HeaderMap, Value) {
    let req = common::auth::json(
        Method::POST,
        &format!("/v1/exports/{}/share-link", export["id"].as_str().unwrap()),
        &body,
        Some(bearer),
    );
    common::send(c.app.clone(), req).await
}

/// Creates a link and returns `(link id, passcode)`.
async fn link(c: &Ctx, bearer: &str, export: &Value, body: Value) -> (String, String) {
    let (s, _, b) = create_link(c, bearer, export, body).await;
    assert_eq!(s, StatusCode::CREATED, "{b}");
    (
        b["link"]["id"].as_str().unwrap().to_owned(),
        b["passcode"].as_str().unwrap().to_owned(),
    )
}

/// `POST /v1/share/{id}/open` without a bearer, from `ip`.
async fn open(c: &Ctx, id: &str, passcode: &str, ip: &str) -> (StatusCode, HeaderMap, Value) {
    let req = common::auth::json(
        Method::POST,
        &format!("/v1/share/{id}/open"),
        &json!({"passcode": passcode}),
        None,
    );
    common::send(c.app.clone(), from_ip(req, ip)).await
}

/// `GET /v1/share/{id}/file?t=`: status, headers and raw bytes.
async fn download(app: axum::Router, id: &str, token: &str) -> (StatusCode, HeaderMap, Vec<u8>) {
    let req = Request::builder()
        .uri(format!("/v1/share/{id}/file?t={token}"))
        .body(Body::empty())
        .unwrap();
    let res = app.oneshot(from_ip(req, "198.51.100.9")).await.unwrap();
    let (s, h) = (res.status(), res.headers().clone());
    let bytes = res.into_body().collect().await.unwrap().to_bytes().to_vec();
    (s, h, bytes)
}

fn assert_public_headers(h: &HeaderMap) {
    assert_eq!(h["cache-control"], "no-store", "{h:?}");
    assert_eq!(h["x-robots-tag"], "noindex, nofollow", "{h:?}");
    assert_eq!(h["referrer-policy"], "no-referrer", "{h:?}");
}

#[tokio::test]
async fn create_returns_the_passcode_once_and_the_list_never() {
    let _serial = SERIAL.lock().await;
    let Some((c, clin, export)) = setup().await else {
        return;
    };
    let (s, h, made) = create_link(&c, &clin, &export, json!({})).await;
    assert_eq!(s, StatusCode::CREATED, "{made}");
    assert_eq!(
        h["cache-control"], "no-store",
        "the passcode is never cached"
    );
    let passcode = made["passcode"].as_str().unwrap().to_owned();
    assert_eq!(passcode.len(), 12);
    assert!(passcode
        .bytes()
        .all(|b| SHARE_PASSCODE_ALPHABET.contains(&b)));
    let link = &made["link"];
    let id = link["id"].as_str().unwrap();
    assert_eq!(
        link["url"],
        format!("/share/{id}"),
        "relative without a console origin"
    );
    assert_eq!(link["export_id"], export["id"]);
    assert_eq!(link["max_downloads"], 1, "owner decision 5 default");
    assert_eq!(link["downloads"], 0);
    // Default 72 h, never past the artifact (whose TTL is also 72 h here).
    let expires: chrono::DateTime<chrono::Utc> =
        link["expires_at"].as_str().unwrap().parse().unwrap();
    let artifact: chrono::DateTime<chrono::Utc> =
        export["expires_at"].as_str().unwrap().parse().unwrap();
    assert!(expires <= artifact, "{expires} > {artifact}");
    assert!(expires > chrono::Utc::now() + chrono::Duration::hours(71));

    let stored: String = sqlx::query_scalar("SELECT passcode_hash FROM share_links WHERE id = $1")
        .bind(Uuid::parse_str(id).unwrap())
        .fetch_one(&c.db.pool)
        .await
        .unwrap();
    assert!(
        stored.starts_with("$argon2id$v=19$m=19456,t=2,p=1$"),
        "{stored}"
    );
    assert!(!stored.contains(&passcode));

    let (s, list) = c
        .call(
            Method::GET,
            &format!("/v1/exports/{}/share-links", export["id"].as_str().unwrap()),
            None,
            &clin,
        )
        .await;
    assert_eq!(s, StatusCode::OK, "{list}");
    assert_eq!(list["items"].as_array().unwrap().len(), 1);
    assert!(list["items"][0].get("passcode").is_none());
    assert!(!list.to_string().contains(&passcode));
    let (_, again) = c
        .call(
            Method::GET,
            &format!("/v1/exports/{}", export["id"].as_str().unwrap()),
            None,
            &clin,
        )
        .await;
    assert_eq!(again["share_links_n"], 1);

    let audit = c.audit_rows("share_link.create").await;
    assert_eq!(audit.len(), 1);
    assert_eq!(audit[0]["target_id"], id);
    assert_eq!(audit[0]["after"]["export_id"], export["id"]);
    assert!(!c.audit_dump().await.contains(&passcode));
    c.db.cleanup().await;
}

#[tokio::test]
async fn create_is_refused_out_of_role_org_format_and_range() {
    let _serial = SERIAL.lock().await;
    let Some((c, clin, export)) = setup().await else {
        return;
    };
    let (_, researcher) = c.token(Role::Researcher).await;
    let (_, admin) = c.token(Role::Admin).await;
    let (_, root) = c.token(Role::Superadmin).await;
    for bearer in [&researcher, &admin, &root] {
        let (s, _, b) = create_link(&c, bearer, &export, json!({})).await;
        assert!(
            s == StatusCode::FORBIDDEN || s == StatusCode::NOT_FOUND,
            "{s}: {b}"
        );
    }
    let other_org = common::auth::seed_org(&c.db.pool, "Clinic B").await;
    let (_, outsider) = c.token_in(Some(other_org), Role::Clinician).await;
    let (s, _, _) = create_link(&c, &outsider, &export, json!({})).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    for body in [
        json!({"max_downloads": 0}),
        json!({"max_downloads": 6}),
        json!({"expires_in_days": 0}),
        json!({"expires_in_days": 8}),
    ] {
        let (s, _, b) = create_link(&c, &clin, &export, body.clone()).await;
        assert_eq!(s, StatusCode::BAD_REQUEST, "{body}: {b}");
        assert_eq!(slug_of(&b), "validation-failed");
    }
    // Only case packages are shared this way.
    let csv = c
        .export_and_run(
            &clin,
            export_support::body("csv", "identified", "clinical_care"),
        )
        .await;
    let (s, _, b) = create_link(&c, &clin, &csv, json!({})).await;
    assert_eq!(s, StatusCode::CONFLICT, "{b}");
    // An expired artifact cannot be shared.
    sqlx::query("UPDATE exports SET status = 'expired' WHERE id = $1")
        .bind(Uuid::parse_str(export["id"].as_str().unwrap()).unwrap())
        .execute(&c.db.pool)
        .await
        .unwrap();
    let (s, _, b) = create_link(&c, &clin, &export, json!({})).await;
    assert_eq!(s, StatusCode::GONE, "{b}");
    assert_eq!(slug_of(&b), "export-expired");
    assert!(c.audit_rows("share_link.create").await.is_empty());
    c.db.cleanup().await;
}

#[tokio::test]
async fn opening_checks_the_passcode_and_audits_with_the_ip() {
    let _serial = SERIAL.lock().await;
    let Some((c, clin, export)) = setup().await else {
        return;
    };
    let (id, passcode) = link(&c, &clin, &export, json!({"max_downloads": 2})).await;

    let wrong = if passcode.starts_with('A') { "B" } else { "A" }.to_owned() + &passcode[1..];
    let (s, h, b) = open(&c, &id, &wrong, "203.0.113.7").await;
    assert_eq!(s, StatusCode::UNAUTHORIZED, "{b}");
    assert_eq!(slug_of(&b), "share-passcode-invalid");
    assert_public_headers(&h);
    let failed = c.audit_rows("share_link.open_failed").await;
    assert_eq!(failed.len(), 1);
    assert_eq!(failed[0]["actor_type"], "system");
    assert_eq!(failed[0]["ip"], "203.0.113.7");
    assert_eq!(failed[0]["target_id"], id);
    assert_eq!(failed[0]["org_id"], c.org.to_string());

    // Typed loosely (lower case, grouped): still the passcode.
    let typed = format!("{}-{}", &passcode[..6], &passcode[6..]).to_lowercase();
    let (s, h, opened) = open(&c, &id, &typed, "203.0.113.8").await;
    assert_eq!(s, StatusCode::OK, "{opened}");
    assert_public_headers(&h);
    assert_eq!(opened["export_id"], export["id"]);
    assert_eq!(opened["file_name"], export["file_name"]);
    assert_eq!(opened["bytes"], export["byte_size"]);
    assert_eq!(opened["downloads_left"], 2);
    assert!(opened["download_token"].as_str().unwrap().len() > 20);
    let opens = c.audit_rows("share_link.open").await;
    assert_eq!(opens.len(), 1);
    assert_eq!(opens[0]["ip"], "203.0.113.8");
    assert_eq!(opens[0]["after"]["export_id"], export["id"]);
    let (n, ip): (i32, Option<String>) =
        sqlx::query_as("SELECT opens, host(last_opened_ip) FROM share_links WHERE id = $1")
            .bind(Uuid::parse_str(&id).unwrap())
            .fetch_one(&c.db.pool)
            .await
            .unwrap();
    assert_eq!((n, ip.as_deref()), (1, Some("203.0.113.8")));

    // The token works for this link only; tampered or missing tokens are 403.
    let token = opened["download_token"].as_str().unwrap();
    let (s, h, _) = download(c.app.clone(), &id, "1.AAAA").await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    assert_public_headers(&h);
    let (other, _) = link(&c, &clin, &export, json!({})).await;
    let (s, _, _) = download(c.app.clone(), &other, token).await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    let (s, h, bytes) = download(c.app.clone(), &id, token).await;
    assert_eq!(s, StatusCode::OK);
    assert_public_headers(&h);
    assert!(h["content-disposition"]
        .to_str()
        .unwrap()
        .starts_with("attachment; filename=\"export-"));
    assert_eq!(h["content-type"], "application/zip");
    assert_eq!(export["sha256"], hex(&Sha256::digest(&bytes)));
    let downloads = c.audit_rows("share_link.download").await;
    assert_eq!(downloads.len(), 1);
    assert_eq!(downloads[0]["ip"], "198.51.100.9");
    assert_eq!(downloads[0]["target_type"], "share_link");
    assert_eq!(downloads[0]["after"]["export_id"], export["id"]);
    let dump = c.audit_dump().await;
    assert!(!dump.contains(&passcode) && !dump.contains(token));
    c.db.cleanup().await;
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

#[tokio::test]
async fn downloads_are_counted_atomically_under_concurrency() {
    let _serial = SERIAL.lock().await;
    let Some((c, clin, export)) = setup().await else {
        return;
    };
    let (id, passcode) = link(&c, &clin, &export, json!({"max_downloads": 3})).await;
    let (s, _, opened) = open(&c, &id, &passcode, "203.0.113.7").await;
    assert_eq!(s, StatusCode::OK, "{opened}");
    let token = opened["download_token"].as_str().unwrap().to_owned();

    let tasks: Vec<_> = (0..10)
        .map(|_| {
            let (app, id, token) = (c.app.clone(), id.clone(), token.clone());
            tokio::spawn(async move { download(app, &id, &token).await })
        })
        .collect();
    let mut ok = 0;
    let mut exhausted = 0;
    for t in tasks {
        let (s, h, body) = t.await.unwrap();
        assert_public_headers(&h);
        match s {
            StatusCode::OK => ok += 1,
            StatusCode::GONE => {
                let b: Value = serde_json::from_slice(&body).unwrap();
                assert_eq!(slug_of(&b), "share-link-exhausted", "{b}");
                exhausted += 1;
            }
            other => panic!("unexpected {other}"),
        }
    }
    assert_eq!((ok, exhausted), (3, 7));
    let n: i32 = sqlx::query_scalar("SELECT downloads FROM share_links WHERE id = $1")
        .bind(Uuid::parse_str(&id).unwrap())
        .fetch_one(&c.db.pool)
        .await
        .unwrap();
    assert_eq!(n, 3);
    assert_eq!(c.audit_rows("share_link.download").await.len(), 3);
    // Spent: opening says so before asking for the passcode.
    let (s, _, b) = open(&c, &id, &passcode, "203.0.113.7").await;
    assert_eq!(s, StatusCode::GONE, "{b}");
    assert_eq!(slug_of(&b), "share-link-exhausted");
    c.db.cleanup().await;
}

#[tokio::test]
async fn revoked_and_unknown_links_are_404_and_expired_ones_410() {
    let _serial = SERIAL.lock().await;
    let Some((c, clin, export)) = setup().await else {
        return;
    };
    let (id, passcode) = link(&c, &clin, &export, json!({"max_downloads": 5})).await;
    let (s, _, opened) = open(&c, &id, &passcode, "203.0.113.7").await;
    assert_eq!(s, StatusCode::OK);
    let token = opened["download_token"].as_str().unwrap().to_owned();

    for unknown in [Uuid::new_v4().to_string(), "not-a-uuid".to_owned()] {
        let (s, h, _) = open(&c, &unknown, &passcode, "203.0.113.7").await;
        assert_eq!(s, StatusCode::NOT_FOUND);
        assert_public_headers(&h);
        let (s, _, _) = download(c.app.clone(), &unknown, &token).await;
        assert_eq!(s, StatusCode::NOT_FOUND);
    }

    // Expired link: 410 share-link-expired, on open and on download.
    let link_id = Uuid::parse_str(&id).unwrap();
    sqlx::query(
        "UPDATE share_links SET created_at = now() - interval '2 days',
                                expires_at = now() - interval '1 minute' WHERE id = $1",
    )
    .bind(link_id)
    .execute(&c.db.pool)
    .await
    .unwrap();
    let (s, _, b) = open(&c, &id, &passcode, "203.0.113.7").await;
    assert_eq!(s, StatusCode::GONE, "{b}");
    assert_eq!(slug_of(&b), "share-link-expired");
    let (s, _, body) = download(c.app.clone(), &id, &token).await;
    assert_eq!(s, StatusCode::GONE);
    assert_eq!(
        slug_of(&serde_json::from_slice(&body).unwrap()),
        "share-link-expired"
    );

    // Export gone (EXJ-4) under a live link: 410 export-expired once the passcode matches.
    let (live, live_pass) = link(&c, &clin, &export, json!({})).await;
    sqlx::query("UPDATE exports SET status = 'expired' WHERE id = $1")
        .bind(Uuid::parse_str(export["id"].as_str().unwrap()).unwrap())
        .execute(&c.db.pool)
        .await
        .unwrap();
    let (s, _, b) = open(&c, &live, &live_pass, "203.0.113.7").await;
    assert_eq!(s, StatusCode::GONE, "{b}");
    assert_eq!(slug_of(&b), "export-expired");

    // Revoked: 404, idempotent, audited once.
    let (s, _) = c
        .call(
            Method::DELETE,
            &format!("/v1/share-links/{live}"),
            None,
            &clin,
        )
        .await;
    assert_eq!(s, StatusCode::NO_CONTENT);
    let (s, _) = c
        .call(
            Method::DELETE,
            &format!("/v1/share-links/{live}"),
            None,
            &clin,
        )
        .await;
    assert_eq!(s, StatusCode::NO_CONTENT);
    let revokes = c.audit_rows("share_link.revoke").await;
    assert_eq!(revokes.len(), 1);
    assert_eq!(revokes[0]["after"]["export_id"], export["id"]);
    let (s, _, _) = open(&c, &live, &live_pass, "203.0.113.7").await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    let (_, researcher) = c.token(Role::Researcher).await;
    let (s, _) = c
        .call(
            Method::DELETE,
            &format!("/v1/share-links/{id}"),
            None,
            &researcher,
        )
        .await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    // The list still shows revoked links, with their counters.
    let (_, list) = c
        .call(
            Method::GET,
            &format!("/v1/exports/{}/share-links", export["id"].as_str().unwrap()),
            None,
            &clin,
        )
        .await;
    let items = list["items"].as_array().unwrap();
    assert_eq!(items.len(), 2);
    assert!(items
        .iter()
        .any(|l| l["id"] == live && l["revoked_at"].is_string()));
    assert!(items.iter().any(|l| l["id"] == id && l["opens"] == 1));
    c.db.cleanup().await;
}

#[tokio::test]
async fn opens_are_limited_per_link_and_per_ip() {
    let _serial = SERIAL.lock().await;
    let Some((c, clin, export)) = setup().await else {
        return;
    };
    let mut links = Vec::new();
    for _ in 0..5 {
        links.push(link(&c, &clin, &export, json!({})).await);
    }
    // Per link: 10 an hour, whichever IP asks (a wrong-length passcode costs no Argon2).
    let (first, _) = &links[0];
    for i in 0..10 {
        let (s, _, b) = open(&c, first, "x", &format!("192.0.2.{i}")).await;
        assert_eq!(s, StatusCode::UNAUTHORIZED, "{i}: {b}");
    }
    let (s, h, b) = open(&c, first, "x", "192.0.2.99").await;
    assert_eq!(s, StatusCode::TOO_MANY_REQUESTS, "{b}");
    assert!(h["retry-after"].to_str().unwrap().parse::<u64>().unwrap() > 0);
    assert_public_headers(&h);
    // The right passcode does not get past the limit either.
    let (s, _, _) = open(&c, first, &links[0].1, "192.0.2.98").await;
    assert_eq!(s, StatusCode::TOO_MANY_REQUESTS);

    // Per IP: 30 an hour over every link (three links at their own limit of ten).
    for (id, _) in &links[1..4] {
        for _ in 0..10 {
            let (s, _, b) = open(&c, id, "x", "198.51.100.1").await;
            assert_eq!(s, StatusCode::UNAUTHORIZED, "{b}");
        }
    }
    let (s, _, b) = open(&c, &links[4].0, &links[4].1, "198.51.100.1").await;
    assert_eq!(
        s,
        StatusCode::TOO_MANY_REQUESTS,
        "31st open from one IP: {b}"
    );
    let (s, _, b) = open(&c, &links[4].0, &links[4].1, "198.51.100.2").await;
    assert_eq!(s, StatusCode::OK, "another IP still opens that link: {b}");
    c.db.cleanup().await;
}

// ---- SHR-11: erasure revokes the links of every package it expires --------------------------

/// Files an erasure of `participant` as an org admin and runs the queue; returns the
/// request row.
async fn erase(c: &Ctx, participant: &str) -> Value {
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
    sqlx::query_scalar(
        "SELECT to_jsonb(r) FROM erasure_requests r WHERE participant_id = $1::uuid
         ORDER BY requested_at DESC LIMIT 1",
    )
    .bind(participant)
    .fetch_one(&c.db.pool)
    .await
    .unwrap()
}

async fn revoked_at(c: &Ctx, link: &str) -> Option<chrono::DateTime<chrono::Utc>> {
    sqlx::query_scalar("SELECT revoked_at FROM share_links WHERE id = $1::uuid")
        .bind(link)
        .fetch_one(&c.db.pool)
        .await
        .unwrap()
}

async fn export_status(c: &Ctx, export: &Value) -> String {
    sqlx::query_scalar("SELECT status FROM exports WHERE id = $1::uuid")
        .bind(export["id"].as_str().unwrap())
        .fetch_one(&c.db.pool)
        .await
        .unwrap()
}

#[tokio::test]
async fn erasure_revokes_the_share_links_of_the_packages_it_expires() {
    let _serial = SERIAL.lock().await;
    let Some((c, clin, export)) = setup().await else {
        return;
    };
    let participant = export["case_participant_id"].as_str().unwrap().to_owned();
    let (id, passcode) = link(&c, &clin, &export, json!({"max_downloads": 5})).await;
    let (spare, spare_pass) = link(&c, &clin, &export, json!({})).await;
    // A link the sender revoked already is not revoked (nor audited) again.
    let (s, _) = c
        .call(
            Method::DELETE,
            &format!("/v1/share-links/{spare}"),
            None,
            &clin,
        )
        .await;
    assert_eq!(s, StatusCode::NO_CONTENT);
    let spare_revoked = revoked_at(&c, &spare).await;
    // An unrelated package (another participant) keeps its link.
    let other = c.participant(c.org, "TOKEN-UNRELATED").await;
    c.exportable_session(
        other,
        "TOKEN-UNRELATED",
        "SESS-UNRELATED",
        "2026-10-04T10:00:00Z",
        None,
    )
    .await;
    let unrelated = c.ready_case(&clin, other).await;
    let (kept, kept_pass) = link(&c, &clin, &unrelated, json!({})).await;
    let (s, _, _) = open(&c, &id, &passcode, "203.0.113.7").await;
    assert_eq!(s, StatusCode::OK, "the link works before the erasure");

    let req = erase(&c, &participant).await;
    // The fixture's clinical note holds the record; the package is expired either way.
    assert_eq!(req["status"], "held", "{req}");
    assert_eq!(req["outcome"]["export_artifacts_deleted"], 1, "{req}");
    assert_eq!(export_status(&c, &export).await, "expired");
    assert!(
        revoked_at(&c, &id).await.is_some(),
        "the live link is revoked"
    );
    assert_eq!(revoked_at(&c, &spare).await, spare_revoked);
    assert!(revoked_at(&c, &kept).await.is_none());
    assert_eq!(export_status(&c, &unrelated).await, "succeeded");

    let revokes = c.audit_rows("share_link.revoke").await;
    let by_erasure: Vec<&Value> = revokes
        .iter()
        .filter(|r| r["after"]["reason"] == "erasure")
        .collect();
    assert_eq!(by_erasure.len(), 1, "{revokes:?}");
    let row = by_erasure[0];
    assert_eq!(row["actor_type"], "system");
    assert_eq!(row["target_type"], "share_link");
    assert_eq!(row["target_id"], id);
    assert_eq!(row["org_id"], c.org.to_string());
    assert_eq!(row["after"]["export_id"], export["id"]);
    assert_eq!(row["after"]["erasure_request_id"], req["id"]);

    // Public open: not available, like any revoked link (one 404 for unknown and revoked).
    let (s, h, _) = open(&c, &id, &passcode, "203.0.113.7").await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    assert_public_headers(&h);
    let (s, _, _) = open(&c, &spare, &spare_pass, "203.0.113.7").await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    let (s, _, _) = open(&c, &kept, &kept_pass, "203.0.113.7").await;
    assert_eq!(s, StatusCode::OK, "the unrelated link still opens");
    c.db.cleanup().await;
}

#[tokio::test]
async fn erasure_finds_a_case_package_through_its_participant_alone() {
    let _serial = SERIAL.lock().await;
    let Some(c) = Ctx::new().await else { return };
    let (_, clin) = c.token(Role::Clinician).await;
    // No note and no MRN: the erasure completes.
    let p = c.participant(c.org, "TOKEN-BARE").await;
    c.exportable_session(p, "TOKEN-BARE", "SESS-BARE", "2026-10-01T10:00:00Z", None)
        .await;
    let export = c.ready_case(&clin, p).await;
    let (id, passcode) = link(&c, &clin, &export, json!({})).await;
    // A package with no item naming the participant (no included session) is still found
    // through `exports.case_participant_id`.
    sqlx::query("DELETE FROM export_items WHERE export_id = $1::uuid")
        .bind(export["id"].as_str().unwrap())
        .execute(&c.db.pool)
        .await
        .unwrap();

    let req = erase(&c, &p.to_string()).await;
    assert_eq!(req["status"], "completed", "{req}");
    assert_eq!(req["outcome"]["export_artifacts_deleted"], 1, "{req}");
    assert_eq!(export_status(&c, &export).await, "expired");
    assert!(revoked_at(&c, &id).await.is_some());
    let revokes = c.audit_rows("share_link.revoke").await;
    assert_eq!(revokes.len(), 1);
    assert_eq!(revokes[0]["after"]["reason"], "erasure");
    let (s, _, _) = open(&c, &id, &passcode, "203.0.113.7").await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    c.db.cleanup().await;
}

// ---- OBS-1: neither the passcode nor the token reaches the log --------------------------

#[derive(Clone, Default)]
struct Capture(Arc<Mutex<Vec<u8>>>);

struct CaptureWriter(Arc<Mutex<Vec<u8>>>);

impl Write for CaptureWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'a> MakeWriter<'a> for Capture {
    type Writer = CaptureWriter;
    fn make_writer(&'a self) -> Self::Writer {
        CaptureWriter(self.0.clone())
    }
}

#[test]
fn logs_contain_neither_the_passcode_nor_the_token() {
    let _serial = SERIAL.blocking_lock();
    let cap = Capture::default();
    let seen: Arc<Mutex<Vec<String>>> = Arc::default();
    let seen_in = seen.clone();
    let subscriber = tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new("trace"))
        .with(RedactingJsonLayer::new(cap.clone()));
    tracing::subscriber::with_default(subscriber, || {
        tracing::callsite::rebuild_interest_cache();
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async move {
                let Some((c, clin, export)) = setup().await else {
                    return;
                };
                let (id, passcode) = link(&c, &clin, &export, json!({})).await;
                let wrong = "ZZZZZZZZZZZZ".to_owned();
                open(&c, &id, &wrong, "203.0.113.7").await;
                let (s, _, opened) = open(&c, &id, &passcode, "203.0.113.7").await;
                assert_eq!(s, StatusCode::OK, "{opened}");
                let token = opened["download_token"].as_str().unwrap().to_owned();
                let (s, _, _) = download(c.app.clone(), &id, &token).await;
                assert_eq!(s, StatusCode::OK);
                // A second download is refused (and logged as a response) too.
                download(c.app.clone(), &id, &token).await;
                seen_in.lock().unwrap().extend([passcode, wrong, token]);
                c.db.cleanup().await;
            });
    });
    let needles = seen.lock().unwrap().clone();
    if needles.is_empty() {
        return; // skipped: no database
    }
    let text = String::from_utf8(cap.0.lock().unwrap().clone()).unwrap();
    assert!(
        !text.is_empty(),
        "nothing was logged; the test would be vacuous"
    );
    assert!(
        text.contains("/v1/share/{id}/file"),
        "route templates are logged"
    );
    for needle in &needles {
        assert!(
            !text.contains(needle.as_str()),
            "`{needle}` leaked into the log"
        );
    }
}
