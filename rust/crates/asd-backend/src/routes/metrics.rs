//! owner: WP-B.3
//!
//! `GET /v1/orgs/{id}/metrics` (console dashboard, `api_v1::Metrics`):
//! sessions started in the last 7 / 30 days, all-time distributions by stored attention
//! flag and quality verdict, open review items (`open` + `in_review`), active devices
//! (not revoked, posted a session in the last 30 days) and bundle adoption (each
//! non-revoked device counted once, on the bundle version it fetched last).

use std::collections::BTreeMap;

use asd_contract::api_v1::{BundleAdoption, Metrics};
use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};

use crate::app::AppState;
use crate::auth::extractors::Require;
use crate::auth::rbac::perm;
use crate::error::AppResult;

pub fn router() -> Router<AppState> {
    Router::new().route("/orgs/{id}/metrics", get(metrics))
}

fn count(n: i64) -> u64 {
    u64::try_from(n).unwrap_or(0)
}

async fn metrics(
    auth: Require<perm::MetricsRead>,
    State(state): State<AppState>,
) -> AppResult<Json<Metrics>> {
    let org = auth.org();
    let db = &state.db;

    let recent = sqlx::query!(
        r#"SELECT count(*) FILTER (WHERE started_at >= now() - interval '7 days') AS "d7!",
                  count(*) FILTER (WHERE started_at >= now() - interval '30 days') AS "d30!"
           FROM sessions WHERE org_id = $1"#,
        org
    )
    .fetch_one(db)
    .await?;

    let flags = sqlx::query!(
        r#"SELECT attention_flag, count(*) AS "n!" FROM sessions WHERE org_id = $1
           GROUP BY attention_flag"#,
        org
    )
    .fetch_all(db)
    .await?;
    let quality = sqlx::query!(
        r#"SELECT quality_verdict, count(*) AS "n!" FROM sessions WHERE org_id = $1
           GROUP BY quality_verdict"#,
        org
    )
    .fetch_all(db)
    .await?;

    let queue = sqlx::query_scalar!(
        r#"SELECT count(*) AS "n!" FROM review_items
           WHERE org_id = $1 AND status IN ('open', 'in_review')"#,
        org
    )
    .fetch_one(db)
    .await?;

    let devices = sqlx::query_scalar!(
        r#"SELECT count(*) AS "n!" FROM devices d
           WHERE d.org_id = $1 AND d.revoked_at IS NULL
             AND EXISTS (SELECT 1 FROM sessions s
                         WHERE s.device_id = d.id AND s.received_at >= now() - interval '30 days')"#,
        org
    )
    .fetch_one(db)
    .await?;

    let adoption = sqlx::query!(
        r#"SELECT latest.bundle_version AS "bundle_version!", count(*) AS "devices!"
           FROM (SELECT DISTINCT ON (a.device_id) a.device_id, a.bundle_version
                 FROM bundle_adoption a JOIN devices d ON d.id = a.device_id
                 WHERE d.org_id = $1 AND d.revoked_at IS NULL
                 ORDER BY a.device_id, a.at DESC) latest
           GROUP BY latest.bundle_version
           ORDER BY latest.bundle_version"#,
        org
    )
    .fetch_all(db)
    .await?;

    Ok(Json(Metrics {
        sessions_7d: count(recent.d7),
        sessions_30d: count(recent.d30),
        flag_distribution: flags
            .into_iter()
            .map(|r| (r.attention_flag, count(r.n)))
            .collect::<BTreeMap<_, _>>(),
        quality_distribution: quality
            .into_iter()
            .map(|r| (r.quality_verdict, count(r.n)))
            .collect::<BTreeMap<_, _>>(),
        review_queue_size: count(queue),
        devices_active: count(devices),
        bundle_adoption: adoption
            .into_iter()
            .map(|r| BundleAdoption {
                bundle_version: r.bundle_version,
                devices: count(r.devices),
            })
            .collect(),
    }))
}
