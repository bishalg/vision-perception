//! owner: WP-C.10
//!
//! CW-1, CW-2, RBAC-4 for the worklist: `GET /v1/orgs/{id}/worklist` lists the org's
//! sessions whose clinical review is not `final`, newest received first, with the review
//! state; scope `all` vs `flagged` comes from org settings; synthetic sessions only with
//! `ASD_ALLOW_SYNTHETIC`; another org answers 404 and roles without `WorklistRead` 403.
//! Needs `DATABASE_URL`; skipped otherwise.

mod clinical_support;
mod common;

use clinical_support::*;
use http::StatusCode;
use serde_json::{json, Value};

fn ids(body: &Value) -> Vec<String> {
    body["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["id"].as_str().unwrap().to_owned())
        .collect()
}

async fn worklist(ctx: &Ctx, token: &str, query: &str) -> (StatusCode, Value) {
    let (status, _, body) =
        get_as(ctx, token, &format!("/v1/orgs/{}/worklist{query}", ctx.org)).await;
    (status, body)
}

#[tokio::test]
async fn scope_all_vs_flagged() {
    let Some(ctx) = setup().await else { return };
    let pool = ctx.pool();
    session_full(pool, ctx.org, "S-GREEN", "green", "truedepth", 4).await;
    session_full(pool, ctx.org, "S-YELLOW", "yellow", "truedepth", 3).await;
    session_full(pool, ctx.org, "S-RED", "red", "truedepth", 2).await;
    session_full(pool, ctx.org, "S-UNREL", "unreliable", "truedepth", 1).await;

    // Clinic orgs default to `all`, newest received first.
    let (status, body) = worklist(&ctx, &ctx.clin_a.token, "").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(ids(&body), ["S-UNREL", "S-RED", "S-YELLOW", "S-GREEN"]);
    assert_eq!(body["total"], 4);

    set_settings(
        pool,
        ctx.org,
        json!({"clinical": {"worklist_scope": "flagged"}}),
    )
    .await;
    let (_, body) = worklist(&ctx, &ctx.clin_a.token, "").await;
    assert_eq!(ids(&body), ["S-UNREL", "S-RED", "S-YELLOW"], "green leaves");
    assert_eq!(body["total"], 3);

    set_settings(
        pool,
        ctx.org,
        json!({"clinical": {"worklist_scope": "all"}}),
    )
    .await;
    let (_, body) = worklist(&ctx, &ctx.clin_a.token, "").await;
    assert_eq!(ids(&body).len(), 4);
    ctx.cleanup().await;
}

#[tokio::test]
async fn non_clinic_orgs_default_to_flagged() {
    let Some(ctx) = setup().await else { return };
    let pool = ctx.pool();
    sqlx::query("UPDATE organizations SET kind = 'research' WHERE id = $1")
        .bind(ctx.org)
        .execute(pool)
        .await
        .unwrap();
    session(pool, ctx.org, "S-GREEN", "green").await;
    session(pool, ctx.org, "S-RED", "red").await;
    let (_, body) = worklist(&ctx, &ctx.clin_a.token, "").await;
    assert_eq!(ids(&body), ["S-RED"]);
    ctx.cleanup().await;
}

#[tokio::test]
async fn synthetic_sessions_hidden_unless_allowed() {
    let Some(ctx) = setup().await else { return };
    let pool = ctx.pool();
    session_full(pool, ctx.org, "S-SYNTH", "red", "synthetic", 2).await;
    session_full(pool, ctx.org, "S-REAL", "red", "truedepth", 1).await;
    let (_, body) = worklist(&ctx, &ctx.clin_a.token, "").await;
    assert_eq!(ids(&body), ["S-REAL"]);
    assert_eq!(body["total"], 1);

    // Same database, a server started with ASD_ALLOW_SYNTHETIC=true.
    let state = state_with(pool.clone(), &[("ASD_ALLOW_SYNTHETIC", "true")]);
    let app = common::router(state);
    let (status, _, body) = common::send(
        app,
        request(
            http::Method::GET,
            &format!("/v1/orgs/{}/worklist", ctx.org),
            None,
            Some(&ctx.clin_a.token),
            &[],
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(ids(&body), ["S-REAL", "S-SYNTH"]);
    ctx.cleanup().await;
}

#[tokio::test]
async fn access_by_org_and_role() {
    let Some(ctx) = setup().await else { return };
    session(ctx.pool(), ctx.org, "S-1", "red").await;

    let (status, body) = worklist(&ctx, &ctx.other_clin.token, "").await;
    assert_eq!(status, StatusCode::NOT_FOUND, "another org: {body}");

    for (who, token) in [
        ("viewer", &ctx.viewer.token),
        ("reviewer", &ctx.reviewer.token),
        ("researcher", &ctx.researcher.token),
    ] {
        let (status, _) = worklist(&ctx, token, "").await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{who}");
    }
    for (who, token) in [
        ("owner", &ctx.owner.token),
        ("admin", &ctx.admin.token),
        ("clinician", &ctx.clin_a.token),
        ("superadmin", &ctx.superadmin.token),
    ] {
        let (status, body) = worklist(&ctx, token, "").await;
        assert_eq!(status, StatusCode::OK, "{who}: {body}");
        assert_eq!(ids(&body), ["S-1"], "{who}");
    }
    let (status, _, _) = get_as(&ctx, &ctx.clin_a.token, "/v1/orgs/not-a-uuid/worklist").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    ctx.cleanup().await;
}

#[tokio::test]
async fn items_carry_review_state_and_final_reviews_leave() {
    let Some(ctx) = setup().await else { return };
    let pool = ctx.pool();
    set_cosign_policy(pool, ctx.org, "red_flag").await;
    session_full(pool, ctx.org, "S-OPEN", "green", "truedepth", 3).await;
    session_full(pool, ctx.org, "S-RED", "red", "truedepth", 2).await;
    let review = draft_ready(&ctx, "S-DONE", "green").await;

    let (_, body) = worklist(&ctx, &ctx.clin_a.token, "").await;
    assert_eq!(ids(&body), ["S-DONE", "S-RED", "S-OPEN"]);
    let items = body["items"].as_array().unwrap();
    let done = &items[0];
    assert_eq!(done["review_id"], review["id"]);
    assert_eq!(done["clinical_state"], "in_progress");
    assert_eq!(done["assigned_to"], json!(ctx.clin_a.id.to_string()));
    assert_eq!(done["cosign_required"], false);
    assert_eq!(done["has_patient_ref"], false);
    assert_eq!(
        done["participant_token"], "child-S-DONE",
        "SessionSummary is flattened"
    );
    let red = &items[1];
    assert_eq!(red["review_id"], Value::Null);
    assert_eq!(red["clinical_state"], Value::Null);
    assert_eq!(
        red["cosign_required"], true,
        "red_flag policy on a red session"
    );
    let age = items[2]["age_hours"].as_f64().unwrap();
    assert!((2.9..3.2).contains(&age), "{age}");
    for item in items {
        // RBAC-4: no clinical content on worklist items, for anyone.
        assert!(item.get("impression").is_none());
        assert!(item.get("disposition").is_none());
        assert!(item.get("mrn").is_none());
    }

    let (status, _, signed) = sign(&ctx, &ctx.clin_a.token, &review).await;
    assert_eq!(status, StatusCode::OK, "{signed}");
    assert_eq!(signed["state"], "final");
    let (_, body) = worklist(&ctx, &ctx.clin_a.token, "").await;
    assert_eq!(
        ids(&body),
        ["S-RED", "S-OPEN"],
        "final reviews leave the worklist"
    );
    ctx.cleanup().await;
}

#[tokio::test]
async fn filters_and_cursor() {
    let Some(ctx) = setup().await else { return };
    let pool = ctx.pool();
    for (i, flag) in ["green", "red", "yellow", "red", "green"]
        .iter()
        .enumerate()
    {
        session_full(
            pool,
            ctx.org,
            &format!("S-{i}"),
            flag,
            "truedepth",
            10 - i as i64,
        )
        .await;
    }
    // S-4 newest ... S-0 oldest. Claim S-1 for A, S-3 assigned to B.
    claimed(&ctx, &ctx.clin_a.token, "S-1").await;
    let (status, _, _) = call(
        &ctx,
        request(
            http::Method::POST,
            "/v1/sessions/S-3/clinical-review",
            Some(&json!({"assigned_to": ctx.clin_b.id.to_string()})),
            Some(&ctx.clin_a.token),
            &[],
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);

    let t = &ctx.clin_a.token;
    let (_, body) = worklist(&ctx, t, "?flag=red").await;
    assert_eq!(ids(&body), ["S-3", "S-1"]);
    let (_, body) = worklist(&ctx, t, "?assigned=me").await;
    assert_eq!(ids(&body), ["S-1"]);
    let (_, body) = worklist(&ctx, &ctx.clin_b.token, "?assigned=me").await;
    assert_eq!(ids(&body), ["S-3"]);
    let (_, body) = worklist(&ctx, t, "?assigned=unassigned").await;
    assert_eq!(ids(&body), ["S-4", "S-2", "S-0"]);
    let (_, body) = worklist(&ctx, t, "?assigned=any").await;
    assert_eq!(ids(&body).len(), 5);
    // Unclaimed sessions count as `open`.
    let (_, body) = worklist(&ctx, t, "?state=open").await;
    assert_eq!(ids(&body), ["S-4", "S-3", "S-2", "S-1", "S-0"]);
    let (_, body) = worklist(&ctx, t, "?state=in_progress").await;
    assert!(ids(&body).is_empty());

    // Cursor pages of 2, no overlap, stable order.
    let mut seen = Vec::new();
    let mut query = "?limit=2".to_owned();
    loop {
        let (status, body) = worklist(&ctx, t, &query).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        seen.extend(ids(&body));
        match body["next_cursor"].as_str() {
            Some(c) => query = format!("?limit=2&cursor={c}"),
            None => break,
        }
    }
    assert_eq!(seen, ["S-4", "S-3", "S-2", "S-1", "S-0"]);

    for bad in [
        "?state=done",
        "?flag=purple",
        "?assigned=everyone",
        "?limit=0",
        "?cursor=xx",
        "?from=yesterday",
    ] {
        let (status, body) = worklist(&ctx, t, bad).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{bad}: {body}");
    }
    ctx.cleanup().await;
}
