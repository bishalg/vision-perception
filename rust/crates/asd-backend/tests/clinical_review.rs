//! owner: WP-C.10
//!
//! SIGN-1..3, SIGN-9, RBAC-3/4 for the clinical review draft: claim, draft save with
//! `If-Match`, the state machine, encrypted content, role-dependent reads and one audit row
//! per mutation. Signing is in `signing.rs`. Needs `DATABASE_URL`; skipped otherwise.

mod clinical_support;
mod common;

use asd_backend::clinical::review::{transition, ReviewOp};
use asd_contract::clinical::ClinicalReviewState as S;
use clinical_support::*;
use http::{Method, StatusCode};
use serde_json::{json, Value};

// ---- state machine (pure) -------------------------------------------------------------------

#[test]
fn state_machine_table() {
    use ReviewOp::*;
    let ops = [
        SaveDraft,
        Sign {
            cosign_required: false,
        },
        Sign {
            cosign_required: true,
        },
        Cosign,
        Amend,
    ];
    // Expected result per (state, op) in the order of `ops`; None = refused.
    let table: [(S, [Option<S>; 5]); 5] = [
        (S::Open, [Some(S::InProgress), None, None, None, None]),
        (
            S::InProgress,
            [
                Some(S::InProgress),
                Some(S::Final),
                Some(S::AwaitingCosign),
                None,
                None,
            ],
        ),
        (
            S::AwaitingCosign,
            [None, None, None, Some(S::Final), Some(S::Amending)],
        ),
        (S::Final, [None, None, None, None, Some(S::Amending)]),
        (
            S::Amending,
            [
                Some(S::Amending),
                Some(S::Final),
                Some(S::AwaitingCosign),
                None,
                None,
            ],
        ),
    ];
    assert_eq!(table.len(), S::ALL.len(), "every state is in the table");
    for (state, expected) in table {
        for (op, want) in ops.iter().zip(expected) {
            assert_eq!(transition(state, *op).ok(), want, "{state:?} + {op:?}");
        }
    }
}

// ---- claim ----------------------------------------------------------------------------------

#[tokio::test]
async fn claim_creates_an_open_review_once() {
    let Some(ctx) = setup().await else { return };
    session(ctx.pool(), ctx.org, "S-1", "red").await;

    let (status, headers, review) = claim(&ctx, &ctx.clin_a.token, "S-1").await;
    assert_eq!(status, StatusCode::CREATED, "{review}");
    assert_eq!(review["state"], "open");
    assert_eq!(review["session_id"], "S-1");
    assert_eq!(review["org_id"], json!(ctx.org.to_string()));
    assert_eq!(review["assigned_to"], json!(ctx.clin_a.id.to_string()));
    assert_eq!(review["row_version"], 1);
    assert_eq!(headers["etag"], "\"1\"");
    assert_eq!(review["current"]["version_no"], 1);
    assert_eq!(review["current"]["status"], "draft");
    // SIGN-2: nothing is pre-filled from the (red) attention flag.
    assert_eq!(review["current"]["disposition"], Value::Null);
    assert!(review["current"].get("impression").is_none());
    let hash = review["content_hash"].as_str().unwrap();
    assert_eq!(hash.len(), 64);
    assert!(hash
        .bytes()
        .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()));
    assert_eq!(review["signatures"], json!([]));

    // A second claim returns the same review without a second audit row.
    let (status, _, again) = claim(&ctx, &ctx.clin_b.token, "S-1").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(again["id"], review["id"]);
    let id = review["id"].as_str().unwrap();
    let rows = audit_rows(ctx.pool(), "clinical_review.claim", id).await;
    assert_eq!(rows.len(), 1);
    let after = rows[0].1.as_ref().unwrap();
    assert_eq!(after["version_no"], 1);
    assert_eq!(after["content_hash"], json!(hash));

    // GET by id and by session.
    for uri in [
        format!("/v1/clinical-reviews/{id}"),
        "/v1/sessions/S-1/clinical-review".into(),
    ] {
        let (status, _, got) = get_as(&ctx, &ctx.clin_a.token, &uri).await;
        assert_eq!(status, StatusCode::OK, "{uri}: {got}");
        assert_eq!(got["id"], review["id"]);
    }
    ctx.cleanup().await;
}

#[tokio::test]
async fn claim_access_rules() {
    let Some(ctx) = setup().await else { return };
    session(ctx.pool(), ctx.org, "S-1", "green").await;
    session(ctx.pool(), ctx.other_org, "S-OTHER", "green").await;

    let (status, _, _) = claim(&ctx, &ctx.clin_a.token, "S-OTHER").await;
    assert_eq!(status, StatusCode::NOT_FOUND, "another org's session");
    let (status, _, _) = claim(&ctx, &ctx.clin_a.token, "S-NOPE").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    for (who, token) in [
        ("owner", &ctx.owner.token),
        ("admin", &ctx.admin.token),
        ("viewer", &ctx.viewer.token),
        ("reviewer", &ctx.reviewer.token),
        ("superadmin (RBAC-3)", &ctx.superadmin.token),
    ] {
        let (status, _, body) = claim(&ctx, token, "S-1").await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{who}: {body}");
    }
    // Assigning to a user of another org is refused.
    let (status, _, body) = call(
        &ctx,
        request(
            Method::POST,
            "/v1/sessions/S-1/clinical-review",
            Some(&json!({"assigned_to": ctx.other_clin.id.to_string()})),
            Some(&ctx.clin_a.token),
            &[],
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    // An empty body is a claim too.
    let (status, _, body) = call(
        &ctx,
        request(
            Method::POST,
            "/v1/sessions/S-1/clinical-review",
            None,
            Some(&ctx.clin_a.token),
            &[],
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    // Another org's clinician cannot read it.
    let id = body["id"].as_str().unwrap();
    let (status, _, _) = get_as(
        &ctx,
        &ctx.other_clin.token,
        &format!("/v1/clinical-reviews/{id}"),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    ctx.cleanup().await;
}

// ---- draft save (SIGN-2, SIGN-3) ------------------------------------------------------------

#[tokio::test]
async fn draft_save_needs_matching_if_match() {
    let Some(ctx) = setup().await else { return };
    session(ctx.pool(), ctx.org, "S-1", "yellow").await;
    let review = claimed(&ctx, &ctx.clin_a.token, "S-1").await;

    let (status, headers, saved) = patch(
        &ctx,
        &ctx.clin_a.token,
        &review,
        None,
        json!({"disposition": "monitor", "impression": "First look."}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    assert_eq!(saved["state"], "in_progress");
    assert_eq!(saved["row_version"], 2);
    assert_eq!(headers["etag"], "\"2\"");
    assert_eq!(saved["current"]["disposition"], "monitor");
    assert_eq!(saved["current"]["impression"], "First look.");
    assert_ne!(
        saved["content_hash"], review["content_hash"],
        "content is hashed"
    );

    // A second tab still holding row_version 1: 412, nothing changes.
    let (status, _, body) = patch(
        &ctx,
        &ctx.clin_a.token,
        &review,
        None,
        json!({"impression": "Other tab."}),
    )
    .await;
    assert_eq!(status, StatusCode::PRECONDITION_FAILED, "{body}");
    assert_eq!(common::auth::slug(&body), "stale-version");
    let id = review["id"].as_str().unwrap();
    let (_, _, now) = get_as(
        &ctx,
        &ctx.clin_a.token,
        &format!("/v1/clinical-reviews/{id}"),
    )
    .await;
    assert_eq!(now["current"]["impression"], "First look.");
    assert_eq!(now["row_version"], 2);

    // Weak and bare forms of the current version are accepted; a missing header is not.
    let (status, _, _) = patch(
        &ctx,
        &ctx.clin_a.token,
        &saved,
        Some("W/\"2\""),
        json!({"recommendations": "Recapture."}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _, body) = call(
        &ctx,
        request(
            Method::PATCH,
            &format!("/v1/clinical-reviews/{id}"),
            Some(&json!({"impression": "x"})),
            Some(&ctx.clin_a.token),
            &[],
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "missing If-Match: {body}");

    // "" clears a text field; unknown disposition and oversize text are 400.
    let (_, _, cur) = get_as(
        &ctx,
        &ctx.clin_a.token,
        &format!("/v1/clinical-reviews/{id}"),
    )
    .await;
    let cleared = patched(
        &ctx,
        &ctx.clin_a.token,
        &cur,
        json!({"recommendations": ""}),
    )
    .await;
    assert!(
        cleared["current"].get("recommendations").is_none(),
        "{cleared}"
    );
    for bad in [
        json!({"disposition": "autism"}),
        json!({"impression": "x".repeat(8_001)}),
        json!({"assigned_to": ctx.other_clin.id.to_string()}),
    ] {
        let (status, _, body) = patch(&ctx, &ctx.clin_a.token, &cleared, None, bad).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    }
    let ok = patched(
        &ctx,
        &ctx.clin_a.token,
        &cleared,
        json!({"impression": "é".repeat(8_000)}),
    )
    .await;
    assert_eq!(
        ok["current"]["impression"]
            .as_str()
            .unwrap()
            .chars()
            .count(),
        8_000
    );
    ctx.cleanup().await;
}

#[tokio::test]
async fn content_is_encrypted_and_hidden_without_content_read() {
    let Some(ctx) = setup().await else { return };
    session(ctx.pool(), ctx.org, "S-1", "red").await;
    let review = claimed(&ctx, &ctx.clin_a.token, "S-1").await;
    let secret = "Impression-7f3a: limited joint attention";
    let saved = patched(
        &ctx,
        &ctx.clin_a.token,
        &review,
        json!({"disposition": "refer_for_evaluation", "impression": secret, "recommendations": "Refer-91b"}),
    )
    .await;
    let id = saved["id"].as_str().unwrap();

    // KEY-1: only ciphertext in SQL.
    let (imp, rec): (Option<Vec<u8>>, Option<Vec<u8>>) = sqlx::query_as(
        "SELECT impression_ct, recommendations_ct FROM clinical_review_versions WHERE review_id = $1::uuid",
    )
    .bind(id)
    .fetch_one(ctx.pool())
    .await
    .unwrap();
    let imp = imp.expect("impression stored");
    assert!(!String::from_utf8_lossy(&imp).contains("Impression-7f3a"));
    assert!(!String::from_utf8_lossy(&rec.unwrap()).contains("Refer-91b"));

    // RBAC-4: owner and clinicians read content; admin and superadmin get the same JSON
    // without the content keys; viewer/reviewer/researcher cannot read the review at all.
    let uri = format!("/v1/clinical-reviews/{id}");
    for token in [&ctx.owner.token, &ctx.clin_b.token] {
        let (status, _, body) = get_as(&ctx, token, &uri).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["current"]["impression"], secret);
        assert_eq!(body["current"]["recommendations"], "Refer-91b");
    }
    for token in [&ctx.admin.token, &ctx.superadmin.token] {
        let (status, _, body) = get_as(&ctx, token, &uri).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert!(body["current"].get("impression").is_none(), "{body}");
        assert!(body["current"].get("recommendations").is_none());
        assert!(!body.to_string().contains("Impression-7f3a"));
        assert_eq!(body["content_hash"], saved["content_hash"]);
    }
    for token in [
        &ctx.viewer.token,
        &ctx.reviewer.token,
        &ctx.researcher.token,
    ] {
        let (status, _, _) = get_as(&ctx, token, &uri).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
    }

    // SIGN-9: one audit row per save with the hash and version, never the text.
    let rows = audit_rows(ctx.pool(), "clinical_review.update", id).await;
    assert_eq!(rows.len(), 1);
    let after = rows[0].1.as_ref().unwrap();
    assert_eq!(after["version_no"], 1);
    assert_eq!(after["content_hash"], saved["content_hash"]);
    let all: Vec<String> = sqlx::query_scalar(
        "SELECT coalesce(before::text, '') || coalesce(after::text, '') FROM audit_log",
    )
    .fetch_all(ctx.pool())
    .await
    .unwrap();
    for row in all {
        assert!(
            !row.contains("Impression-7f3a") && !row.contains("Refer-91b"),
            "{row}"
        );
    }
    ctx.cleanup().await;
}

#[tokio::test]
async fn write_roles_and_superadmin_carve_out() {
    let Some(ctx) = setup().await else { return };
    session(ctx.pool(), ctx.org, "S-1", "red").await;
    let review = claimed(&ctx, &ctx.clin_a.token, "S-1").await;
    for (who, token) in [
        ("owner", &ctx.owner.token),
        ("admin", &ctx.admin.token),
        ("reviewer", &ctx.reviewer.token),
        ("superadmin", &ctx.superadmin.token),
    ] {
        let (status, _, body) = patch(&ctx, token, &review, None, json!({"impression": "x"})).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{who}: {body}");
    }
    let (status, _, _) = patch(
        &ctx,
        &ctx.other_clin.token,
        &review,
        None,
        json!({"impression": "x"}),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "another org");
    let (status, _, _) = get_as(&ctx, &ctx.clin_a.token, "/v1/clinical-reviews/not-a-uuid").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    ctx.cleanup().await;
}

#[tokio::test]
async fn final_reviews_refuse_draft_saves() {
    let Some(ctx) = setup().await else { return };
    let review = draft_ready(&ctx, "S-1", "green").await;
    let (status, _, signed) = sign(&ctx, &ctx.clin_a.token, &review).await;
    assert_eq!(status, StatusCode::OK, "{signed}");
    assert_eq!(signed["state"], "final");
    let (status, _, body) = patch(
        &ctx,
        &ctx.clin_a.token,
        &signed,
        None,
        json!({"impression": "late edit"}),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    ctx.cleanup().await;
}

#[tokio::test]
async fn clinical_endpoints_answer_503_without_the_kek() {
    let Some(ctx) = setup().await else { return };
    session(ctx.pool(), ctx.org, "S-1", "red").await;
    let review = claimed(&ctx, &ctx.clin_a.token, "S-1").await;
    let state = state_with(ctx.pool().clone(), &[("ASD_DATA_KEK", "")]);
    assert!(state.keyring.is_none());
    let app = common::router(state);
    let id = review["id"].as_str().unwrap();
    let (status, _, body) = common::send(
        app.clone(),
        request(
            Method::PATCH,
            &format!("/v1/clinical-reviews/{id}"),
            Some(&json!({"impression": "x"})),
            Some(&ctx.clin_a.token),
            &[("if-match", "\"1\"")],
        ),
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
    assert_eq!(common::auth::slug(&body), "clinical-crypto-unavailable");
    // The worklist carries no clinical content and keeps working.
    let (status, _, _) = common::send(
        app,
        request(
            Method::GET,
            &format!("/v1/orgs/{}/worklist", ctx.org),
            None,
            Some(&ctx.clin_a.token),
            &[],
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    ctx.cleanup().await;
}
