//! owner: WP-C.11
//!
//! CW-3 participant timeline: one newest-first stream over five tables, paged by an opaque
//! cursor over `(at, key)`.
//!
//! - `session`: a stored session, at `started_at`; summary `{attention_flag}`.
//! - `clinical_review`: the review opening (at `created_at`, summary `{event: "opened",
//!   state}`) and each signature (summary `{event: "signed" | "cosigned", version_no}`).
//! - `note` and `letter`: headers only (kind, status, author, time). The `_ct` columns are
//!   not selected, so a body cannot reach this stream.
//! - `care_event`: the kind and time; the payload is device free text and is not included.
//!
//! `key` is `"{kind}:{id}"`, unique across the five tables, and sorts with `COLLATE "C"`,
//! so rows on one instant have a fixed order and the cursor never skips or repeats one.

use asd_contract::clinical::{TimelineEntry, TimelineEntryKind, TimelinePage};
use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

use super::cursor::{self, Cursor};
use crate::error::AppResult;

/// One page of the timeline of `participant` (already checked to belong to `org`).
pub async fn page(
    db: &PgPool,
    org: Uuid,
    participant: Uuid,
    after: Option<&Cursor>,
    limit: i64,
) -> AppResult<TimelinePage> {
    let (after_at, after_key) = match after {
        Some(c) => (Some(c.at), Some(c.id.as_str())),
        None => (None, None),
    };
    let rows = sqlx::query!(
        r#"WITH t AS (
             SELECT 'session' AS kind, s.id AS id, s.started_at AS at, s.id AS session_id,
                    NULL::uuid AS actor_id, NULL::text AS actor_name,
                    jsonb_build_object('attention_flag', s.attention_flag) AS summary
             FROM sessions s
             WHERE s.participant_id = $1 AND s.org_id = $2
           UNION ALL
             SELECT 'clinical_review', r.id::text, r.created_at, r.session_id,
                    r.created_by, u.display_name,
                    jsonb_build_object('event', 'opened', 'state', r.state)
             FROM clinical_reviews r
             JOIN sessions s ON s.id = r.session_id
             LEFT JOIN users u ON u.id = r.created_by
             WHERE s.participant_id = $1 AND r.org_id = $2
           UNION ALL
             SELECT 'clinical_review', g.id::text, g.signed_at, r.session_id,
                    g.signer_id, g.signer_display_name,
                    jsonb_build_object(
                        'event', CASE g.role WHEN 'sign' THEN 'signed' ELSE 'cosigned' END,
                        'version_no', v.version_no)
             FROM clinical_signatures g
             JOIN clinical_reviews r ON r.id = g.review_id
             JOIN clinical_review_versions v ON v.id = g.version_id
             JOIN sessions s ON s.id = r.session_id
             WHERE s.participant_id = $1 AND g.org_id = $2
           UNION ALL
             SELECT 'note', n.id::text, n.created_at, n.session_id,
                    n.author_id, u.display_name,
                    jsonb_build_object('note_kind', n.kind)
             FROM clinical_notes n
             LEFT JOIN users u ON u.id = n.author_id
             WHERE n.participant_id = $1 AND n.org_id = $2
           UNION ALL
             SELECT 'letter', l.id::text, l.created_at, NULL::text,
                    l.author_id, u.display_name,
                    jsonb_build_object('letter_kind', l.kind, 'status', l.status)
             FROM referral_letters l
             LEFT JOIN users u ON u.id = l.author_id
             WHERE l.participant_id = $1 AND l.org_id = $2
           UNION ALL
             SELECT 'care_event', c.id::text, c.occurred_at, NULL::text,
                    NULL::uuid, NULL::text,
                    jsonb_build_object('care_kind', c.kind)
             FROM care_events c
             WHERE c.participant_id = $1 AND c.org_id = $2
         ), k AS (
             SELECT t.*, (t.kind || ':' || t.id) COLLATE "C" AS key FROM t
         )
         SELECT kind AS "kind!", id AS "id!", at AS "at!", session_id, actor_id, actor_name,
                summary AS "summary!", key AS "key!"
         FROM k
         WHERE ($3::timestamptz IS NULL OR (at, key) < ($3, $4::text COLLATE "C"))
         ORDER BY at DESC, key DESC
         LIMIT $5"#,
        participant,
        org,
        after_at,
        after_key,
        limit + 1,
    )
    .fetch_all(db)
    .await?;

    let limit = usize::try_from(limit).unwrap_or(usize::MAX);
    let has_more = rows.len() > limit;
    let mut last: Option<(DateTime<Utc>, String)> = None;
    let mut items = Vec::with_capacity(rows.len().min(limit));
    for r in rows.into_iter().take(limit) {
        last = Some((r.at, r.key));
        let kind = TimelineEntryKind::parse(&r.kind)
            .ok_or_else(|| crate::error::AppError::Internal("unknown timeline kind".into()))?;
        let summary = match r.summary {
            Value::Object(m) => m,
            _ => serde_json::Map::new(),
        };
        items.push(TimelineEntry {
            kind,
            id: r.id,
            at: cursor::ts(r.at),
            session_id: r.session_id,
            actor_id: r.actor_id.map(|u| u.to_string()),
            actor_display_name: r.actor_name,
            summary,
        });
    }
    let next_cursor = if has_more {
        last.map(|(at, key)| Cursor::new(at, key).encode())
    } else {
        None
    };
    Ok(TimelinePage { items, next_cursor })
}
