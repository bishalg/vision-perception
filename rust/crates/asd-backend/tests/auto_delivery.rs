//! owner: WP-I.7
//!
//! DLV-6 / SIGN-10: `interop::on_review_finalized` creates, in the signing transaction, one
//! identified clinical-care export and one delivery per active production destination with
//! `trigger = on_final_sign`; none for test or manual destinations; a failed delivery, and
//! even a failing outbox insert, leaves the signature `final`. Needs `DATABASE_URL`.

mod clinical_support;
mod common;
mod delivery_support;
mod participant_support;

use asd_backend::interop::netguard;
use asd_backend::jobs::{runner, JobContext};
use clinical_support::*;
use http::{Method, StatusCode};
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

async fn create_destination(ctx: &Ctx, body: Value) -> String {
    let (s, _, b) = call(
        ctx,
        request(
            Method::POST,
            &format!("/v1/orgs/{}/integrations", ctx.org),
            Some(&body),
            Some(&ctx.owner.token),
            &[],
        ),
    )
    .await;
    assert_eq!(s, StatusCode::CREATED, "{b}");
    let id = b["id"].as_str().unwrap().to_owned();
    if body["kind"] == "webhook" {
        let (s, _, b) = call(
            ctx,
            request(
                Method::PUT,
                &format!("/v1/integrations/{id}/secret"),
                Some(&json!({"secret": delivery_support::webhook_secret()})),
                Some(&ctx.owner.token),
                &[],
            ),
        )
        .await;
        assert_eq!(s, StatusCode::OK, "{b}");
    }
    id
}

fn hook(name: &str, url: &str, environment: &str, trigger: &str) -> Value {
    json!({"kind": "webhook", "name": name, "environment": environment,
           "config": {"url": url, "trigger": trigger, "format": "csv"}})
}

async fn auto_exports(
    pool: &PgPool,
    review: &str,
) -> Vec<(Uuid, String, String, String, Vec<Uuid>)> {
    sqlx::query_as(
        "SELECT id, format, mode, purpose, destination_ids FROM exports
         WHERE review_id = $1::uuid ORDER BY created_at, id",
    )
    .bind(review)
    .fetch_all(pool)
    .await
    .unwrap()
}

async fn state_of(pool: &PgPool, review: &str) -> String {
    sqlx::query_scalar("SELECT state FROM clinical_reviews WHERE id = $1::uuid")
        .bind(review)
        .fetch_one(pool)
        .await
        .unwrap()
}

#[tokio::test]
async fn final_sign_enqueues_one_export_and_delivery_per_production_destination() {
    netguard::allow_loopback_for_tests();
    let Some(ctx) = setup_with(&[("ASD_PUBLIC_BASE_URL", delivery_support::PUBLIC_BASE)]).await
    else {
        return;
    };
    let rx = delivery_support::Receiver::start().await;
    let prod_a = create_destination(
        &ctx,
        hook("A", &rx.url("/a"), "production", "on_final_sign"),
    )
    .await;
    let prod_b = create_destination(
        &ctx,
        hook("B", &rx.url("/b"), "production", "on_final_sign"),
    )
    .await;
    let _test_env =
        create_destination(&ctx, hook("T", &rx.url("/t"), "test", "on_final_sign")).await;
    let _manual = create_destination(&ctx, hook("M", &rx.url("/m"), "production", "manual")).await;
    let only_ready = create_destination(
        &ctx,
        json!({"kind": "webhook", "name": "R", "environment": "production",
               "config": {"url": rx.url("/r"), "trigger": "on_final_sign", "format": "csv",
                          "events": ["export.ready"]}}),
    )
    .await;
    let disabled = create_destination(
        &ctx,
        hook("D", &rx.url("/d"), "production", "on_final_sign"),
    )
    .await;
    sqlx::query("UPDATE integration_destinations SET status = 'disabled' WHERE id = $1::uuid")
        .bind(&disabled)
        .execute(ctx.pool())
        .await
        .unwrap();

    let review = draft_ready(&ctx, "S-1", "green").await;
    let rid = review["id"].as_str().unwrap();
    let (s, _, b) = sign(&ctx, &ctx.clin_a.token, &review).await;
    assert!(s.is_success(), "{s} {b}");
    assert_eq!(state_of(ctx.pool(), rid).await, "final");

    let exports = auto_exports(ctx.pool(), rid).await;
    assert_eq!(exports.len(), 2, "one export per eligible destination");
    let mut targets: Vec<String> = exports.iter().map(|e| e.4[0].to_string()).collect();
    targets.sort();
    let mut want = vec![prod_a.clone(), prod_b.clone()];
    want.sort();
    assert_eq!(targets, want);
    assert!(!targets.contains(&only_ready));
    for e in &exports {
        assert_eq!(
            (e.1.as_str(), e.2.as_str(), e.3.as_str()),
            ("csv", "identified", "clinical_care")
        );
    }
    let deliveries: Vec<(String, String)> = sqlx::query_as(
        "SELECT d.event, d.status FROM export_deliveries d JOIN exports e ON e.id = d.export_id
         WHERE e.review_id = $1::uuid",
    )
    .bind(rid)
    .fetch_all(ctx.pool())
    .await
    .unwrap();
    assert_eq!(deliveries.len(), 2);
    assert!(deliveries
        .iter()
        .all(|(ev, st)| ev == "review.finalized" && st == "pending"));
    assert_eq!(
        finalized_rows(ctx.pool()).await,
        2,
        "system export.create audit rows"
    );

    // The worker renders and delivers; both receivers get a review.finalized event.
    let job_ctx = JobContext::from_state(&ctx.state);
    runner::run_pending_in(&job_ctx, "w").await.unwrap();
    let hits = rx.hits();
    assert_eq!(
        hits.len(),
        2,
        "{:?}",
        hits.iter().map(|h| &h.path).collect::<Vec<_>>()
    );
    assert!(hits.iter().all(|h| h.json()["type"] == "review.finalized"));
    ctx.cleanup().await;
}

async fn finalized_rows(pool: &PgPool) -> i64 {
    sqlx::query_scalar(
        "SELECT count(*) FROM audit_log WHERE action = 'export.create' AND actor_type = 'system'",
    )
    .fetch_one(pool)
    .await
    .unwrap()
}

#[tokio::test]
async fn a_failed_delivery_leaves_the_signature_final() {
    netguard::allow_loopback_for_tests();
    let Some(ctx) = setup_with(&[("ASD_PUBLIC_BASE_URL", delivery_support::PUBLIC_BASE)]).await
    else {
        return;
    };
    let rx = delivery_support::Receiver::start().await;
    rx.respond(500, "down");
    create_destination(
        &ctx,
        hook("A", &rx.url("/a"), "production", "on_final_sign"),
    )
    .await;
    let review = draft_ready(&ctx, "S-1", "green").await;
    let rid = review["id"].as_str().unwrap();
    let (s, _, b) = sign(&ctx, &ctx.clin_a.token, &review).await;
    assert!(s.is_success(), "{s} {b}");
    runner::run_pending_in(&JobContext::from_state(&ctx.state), "w")
        .await
        .unwrap();
    let (status, class): (String, Option<String>) = sqlx::query_as(
        "SELECT d.status, d.last_error_class FROM export_deliveries d
         JOIN exports e ON e.id = d.export_id WHERE e.review_id = $1::uuid",
    )
    .bind(rid)
    .fetch_one(ctx.pool())
    .await
    .unwrap();
    assert_eq!(
        (status.as_str(), class.as_deref()),
        ("pending", Some("http-status"))
    );
    assert_eq!(state_of(ctx.pool(), rid).await, "final");
    assert_eq!(signature_count(ctx.pool(), rid).await, 1);
    ctx.cleanup().await;
}

#[tokio::test]
async fn an_outbox_error_rolls_back_to_the_savepoint_and_the_signature_commits() {
    let Some(ctx) = setup().await else { return };
    create_destination(
        &ctx,
        hook(
            "A",
            "https://his.example.org/hook",
            "production",
            "on_final_sign",
        ),
    )
    .await;
    // Make every auto-export insert fail inside the signing transaction.
    sqlx::raw_sql(
        "CREATE FUNCTION refuse_auto_export() RETURNS trigger LANGUAGE plpgsql AS $$
         BEGIN RAISE EXCEPTION 'outbox unavailable'; END $$;
         CREATE TRIGGER refuse_auto_export BEFORE INSERT ON exports
         FOR EACH ROW WHEN (NEW.review_id IS NOT NULL) EXECUTE FUNCTION refuse_auto_export();",
    )
    .execute(ctx.pool())
    .await
    .unwrap();
    let review = draft_ready(&ctx, "S-1", "green").await;
    let rid = review["id"].as_str().unwrap();
    let (s, _, b) = sign(&ctx, &ctx.clin_a.token, &review).await;
    assert!(s.is_success(), "{s} {b}");
    assert_eq!(state_of(ctx.pool(), rid).await, "final");
    assert_eq!(signature_count(ctx.pool(), rid).await, 1);
    assert!(auto_exports(ctx.pool(), rid).await.is_empty());
    let jobs: i64 = sqlx::query_scalar("SELECT count(*) FROM jobs")
        .fetch_one(ctx.pool())
        .await
        .unwrap();
    assert_eq!(jobs, 0, "the export job rolled back with the savepoint");
    ctx.cleanup().await;
}

#[tokio::test]
async fn cosigned_reviews_deliver_once_on_the_final_signature() {
    let Some(ctx) = setup().await else { return };
    set_cosign_policy(ctx.pool(), ctx.org, "always").await;
    create_destination(
        &ctx,
        hook(
            "A",
            "https://his.example.org/hook",
            "production",
            "on_final_sign",
        ),
    )
    .await;
    let review = draft_ready(&ctx, "S-1", "green").await;
    let rid = review["id"].as_str().unwrap();
    let (s, _, signed) = sign(&ctx, &ctx.clin_a.token, &review).await;
    assert!(s.is_success(), "{signed}");
    assert_eq!(state_of(ctx.pool(), rid).await, "awaiting_cosign");
    assert!(
        auto_exports(ctx.pool(), rid).await.is_empty(),
        "not final yet"
    );
    let (_, _, current) = get_as(
        &ctx,
        &ctx.clin_b.token,
        &format!("/v1/clinical-reviews/{rid}"),
    )
    .await;
    let (s, _, b) = cosign(&ctx, &ctx.clin_b.token, &current).await;
    assert!(s.is_success(), "{s} {b}");
    assert_eq!(state_of(ctx.pool(), rid).await, "final");
    let exports = auto_exports(ctx.pool(), rid).await;
    assert_eq!(exports.len(), 1);
    let requested_by: Option<Uuid> =
        sqlx::query_scalar("SELECT requested_by FROM exports WHERE id = $1")
            .bind(exports[0].0)
            .fetch_one(ctx.pool())
            .await
            .unwrap();
    assert_eq!(
        requested_by,
        Some(ctx.clin_b.id),
        "the finaliser requested it"
    );
    ctx.cleanup().await;
}
