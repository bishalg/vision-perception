//! owner: WP-B.2
//!
//! `GET /v1/orgs/{id}`, `PATCH /v1/orgs/{id}` (retention, settings) and
//! `GET /v1/orgs/{id}/audit` (AUTH-5 read side), plus [`listing`]: the cursor/limit
//! parsing shared by the B.2 list routes (users, devices, audit).

use asd_contract::api_v1::{AuditEntry, AuditPage, Org, OrgKind, OrgPatch};
use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};
use serde_json::{json, Map, Value};

use crate::app::AppState;
use crate::audit::AuditEvent;
use crate::auth::extractors::{ApiJson, RequestMeta, Require};
use crate::auth::rbac::perm;
use crate::error::{AppError, AppResult};
use crate::problem::InvalidParam;
use listing::{rfc3339, ListParams};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/orgs/{id}", get(get_org).patch(patch_org))
        .route("/orgs/{id}/audit", get(list_audit))
}

pub const MAX_NAME_LEN: usize = 200;
pub const MAX_RETENTION_MONTHS: u32 = 240;

fn org_kind(kind: &str) -> AppResult<OrgKind> {
    serde_json::from_value(Value::from(kind))
        .map_err(|_| AppError::Internal(format!("unknown org kind in database: {kind}")))
}

fn snapshot(name: &str, retention_months: i32, settings: &Value) -> Value {
    json!({"name": name, "retention_months": retention_months, "settings": settings})
}

async fn get_org(
    State(state): State<AppState>,
    auth: Require<perm::OrgRead>,
) -> AppResult<Json<Org>> {
    let org = auth.org();
    let row = sqlx::query!(
        "SELECT id, name, kind, retention_months FROM organizations WHERE id = $1",
        org
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;
    Ok(Json(Org {
        id: row.id.to_string(),
        name: row.name,
        kind: org_kind(&row.kind)?,
        retention_months: u32::try_from(row.retention_months).ok(),
    }))
}

fn validate_patch(patch: &OrgPatch) -> AppResult<Option<String>> {
    let mut errors = Vec::new();
    let name = patch.name.as_deref().map(str::trim).map(str::to_owned);
    if let Some(n) = &name {
        if n.is_empty() || n.chars().count() > MAX_NAME_LEN {
            errors.push(InvalidParam {
                name: "name".into(),
                reason: format!("must be 1 to {MAX_NAME_LEN} characters"),
            });
        }
    }
    if let Some(r) = patch.retention_months {
        if r == 0 || r > MAX_RETENTION_MONTHS {
            errors.push(InvalidParam {
                name: "retention_months".into(),
                reason: format!("must be 1 to {MAX_RETENTION_MONTHS}"),
            });
        }
    }
    if let Some(settings) = &patch.settings {
        if let Some(rate) = settings.get("audit_rate") {
            // ING-3 random-audit probability.
            if !rate.as_f64().is_some_and(|r| (0.0..=1.0).contains(&r)) {
                errors.push(InvalidParam {
                    name: "settings.audit_rate".into(),
                    reason: "must be a number from 0 to 1".into(),
                });
            }
        }
    }
    if errors.is_empty() {
        Ok(name)
    } else {
        Err(AppError::Validation(errors))
    }
}

async fn patch_org(
    State(state): State<AppState>,
    auth: Require<perm::OrgUpdate>,
    meta: RequestMeta,
    ApiJson(patch): ApiJson<OrgPatch>,
) -> AppResult<Json<Org>> {
    let org = auth.org();
    let name = validate_patch(&patch)?;
    let mut tx = state.db.begin().await?;
    let current = sqlx::query!(
        "SELECT name, kind, retention_months, settings FROM organizations WHERE id = $1 FOR UPDATE",
        org
    )
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(AppError::NotFound)?;

    let new_name = name.unwrap_or_else(|| current.name.clone());
    let new_retention = match patch.retention_months {
        Some(r) => i32::try_from(r).map_err(|_| AppError::Internal("retention".into()))?,
        None => current.retention_months,
    };
    let mut new_settings = match &current.settings {
        Value::Object(m) => m.clone(),
        _ => Map::new(),
    };
    for (k, v) in patch.settings.iter().flatten() {
        new_settings.insert(k.clone(), v.clone());
    }
    let new_settings = Value::Object(new_settings);

    let before = snapshot(&current.name, current.retention_months, &current.settings);
    let after = snapshot(&new_name, new_retention, &new_settings);
    if before != after {
        sqlx::query!(
            "UPDATE organizations SET name = $2, retention_months = $3, settings = $4 WHERE id = $1",
            org,
            new_name,
            new_retention,
            new_settings
        )
        .execute(&mut *tx)
        .await?;
        AuditEvent::user(&auth.principal, "org.update", "organization", org)
            .org(org)
            .before(before)
            .after(after)
            .meta(&meta)
            .record(&mut tx)
            .await?;
    }
    tx.commit().await?;
    Ok(Json(Org {
        id: org.to_string(),
        name: new_name,
        kind: org_kind(&current.kind)?,
        retention_months: u32::try_from(new_retention).ok(),
    }))
}

async fn list_audit(
    State(state): State<AppState>,
    auth: Require<perm::AuditRead>,
    page: ListParams,
) -> AppResult<Json<AuditPage>> {
    let org = auth.org();
    let before_id: Option<i64> = page.cursor_i64()?;
    let limit = page.limit;
    let rows = sqlx::query!(
        "SELECT id, ts, actor_type, actor_id, org_id, action, target_type, target_id, before, after
         FROM audit_log
         WHERE org_id = $1 AND ($2::bigint IS NULL OR id < $2)
         ORDER BY id DESC
         LIMIT $3",
        org,
        before_id,
        i64::from(limit) + 1
    )
    .fetch_all(&state.db)
    .await?;
    let (rows, more) = listing::split_page(rows, limit);
    let next_cursor = more
        .then(|| {
            rows.last()
                .map(|r| listing::encode_cursor(&r.id.to_string()))
        })
        .flatten();
    let items = rows
        .into_iter()
        .map(|r| {
            Ok(AuditEntry {
                id: r.id.to_string(),
                ts: rfc3339(r.ts),
                actor_type: serde_json::from_value(Value::from(r.actor_type))
                    .map_err(|e| AppError::Internal(format!("actor_type: {e}")))?,
                actor_id: r.actor_id,
                org_id: r.org_id.map(|o| o.to_string()),
                action: r.action,
                target_type: Some(r.target_type),
                target_id: r.target_id,
                before: r.before,
                after: r.after,
            })
        })
        .collect::<AppResult<Vec<_>>>()?;
    Ok(Json(AuditPage {
        items,
        next_cursor,
        total: None,
    }))
}

/// `?cursor=&limit=` for the B.2 list routes: `limit` 1..=100 (default 50), `cursor` an
/// opaque base64url string from the previous page's `next_cursor`.
pub mod listing {
    use axum::extract::{FromRequestParts, Query};
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use base64::Engine;
    use chrono::{DateTime, SecondsFormat, Utc};
    use http::request::Parts;
    use serde::Deserialize;
    use uuid::Uuid;

    use crate::error::{AppError, AppResult};
    use crate::problem::InvalidParam;

    pub const DEFAULT_LIMIT: u32 = 50;
    pub const MAX_LIMIT: u32 = 100;

    /// RFC 3339 UTC with milliseconds, as every `/v1` timestamp.
    pub fn rfc3339(ts: DateTime<Utc>) -> String {
        ts.to_rfc3339_opts(SecondsFormat::Millis, true)
    }

    #[derive(Debug, Deserialize)]
    struct Raw {
        cursor: Option<String>,
        limit: Option<String>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ListParams {
        pub cursor: Option<String>,
        pub limit: u32,
    }

    fn invalid(name: &str, reason: &str) -> AppError {
        AppError::Validation(vec![InvalidParam {
            name: name.into(),
            reason: reason.into(),
        }])
    }

    impl<S: Send + Sync> FromRequestParts<S> for ListParams {
        type Rejection = AppError;

        async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, AppError> {
            let Query(raw) = Query::<Raw>::try_from_uri(&parts.uri)
                .map_err(|_| invalid("query", "malformed query string"))?;
            let limit = match raw.limit {
                None => DEFAULT_LIMIT,
                Some(l) => l
                    .parse::<u32>()
                    .ok()
                    .filter(|l| (1..=MAX_LIMIT).contains(l))
                    .ok_or_else(|| invalid("limit", "must be 1 to 100"))?,
            };
            Ok(Self {
                cursor: raw.cursor.filter(|c| !c.is_empty()),
                limit,
            })
        }
    }

    pub fn encode_cursor(position: &str) -> String {
        URL_SAFE_NO_PAD.encode(position.as_bytes())
    }

    fn decoded(cursor: &str) -> AppResult<String> {
        URL_SAFE_NO_PAD
            .decode(cursor)
            .ok()
            .and_then(|b| String::from_utf8(b).ok())
            .ok_or_else(|| invalid("cursor", "not a cursor from this list"))
    }

    /// Keyset position `(created_at, id)` for lists ordered newest first.
    pub fn encode_ts_cursor(ts: DateTime<Utc>, id: Uuid) -> String {
        encode_cursor(&format!("{}|{id}", ts.timestamp_micros()))
    }

    impl ListParams {
        pub fn cursor_i64(&self) -> AppResult<Option<i64>> {
            self.cursor
                .as_deref()
                .map(|c| {
                    decoded(c)?
                        .parse()
                        .map_err(|_| invalid("cursor", "not a cursor from this list"))
                })
                .transpose()
        }

        pub fn cursor_ts(&self) -> AppResult<Option<(DateTime<Utc>, Uuid)>> {
            self.cursor
                .as_deref()
                .map(|c| {
                    let text = decoded(c)?;
                    let bad = || invalid("cursor", "not a cursor from this list");
                    let (micros, id) = text.split_once('|').ok_or_else(bad)?;
                    let ts = micros
                        .parse::<i64>()
                        .ok()
                        .and_then(DateTime::from_timestamp_micros)
                        .ok_or_else(bad)?;
                    Ok((ts, Uuid::parse_str(id).map_err(|_| bad())?))
                })
                .transpose()
        }
    }

    /// Splits the `limit + 1` rows fetched into the page and whether more exist.
    pub fn split_page<T>(mut rows: Vec<T>, limit: u32) -> (Vec<T>, bool) {
        let limit = usize::try_from(limit).unwrap_or(usize::MAX);
        let more = rows.len() > limit;
        rows.truncate(limit);
        (rows, more)
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        async fn params(uri: &str) -> AppResult<ListParams> {
            let (mut parts, ()) = http::Request::builder()
                .uri(uri)
                .body(())
                .unwrap()
                .into_parts();
            ListParams::from_request_parts(&mut parts, &()).await
        }

        #[tokio::test]
        async fn limit_bounds_and_defaults() {
            assert_eq!(params("/x").await.unwrap().limit, 50);
            assert_eq!(params("/x?limit=100").await.unwrap().limit, 100);
            for bad in ["/x?limit=0", "/x?limit=101", "/x?limit=ten"] {
                assert!(params(bad).await.is_err(), "{bad}");
            }
        }

        #[test]
        fn cursors_round_trip_and_reject_garbage() {
            let ts = DateTime::from_timestamp_micros(1_761_000_000_123_456).unwrap();
            let id = Uuid::new_v4();
            let p = ListParams {
                cursor: Some(encode_ts_cursor(ts, id)),
                limit: 2,
            };
            assert_eq!(p.cursor_ts().unwrap(), Some((ts, id)));
            let p = ListParams {
                cursor: Some(encode_cursor("42")),
                limit: 2,
            };
            assert_eq!(p.cursor_i64().unwrap(), Some(42));
            for bad in ["%%%", "bm9wZQ", ""] {
                let p = ListParams {
                    cursor: Some(bad.into()),
                    limit: 2,
                };
                assert!(p.cursor_ts().is_err(), "{bad}");
            }
            assert_eq!(split_page(vec![1, 2, 3], 2), (vec![1, 2], true));
            assert_eq!(split_page(vec![1, 2], 2), (vec![1, 2], false));
        }
    }
}
