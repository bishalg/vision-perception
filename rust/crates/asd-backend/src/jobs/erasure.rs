//! owner: WP-B.9 (attachments and encounters: WP-B.19; case packages and share links:
//! WP-I.13b)
//!
//! The `erasure` job: executes one `erasure_requests` row (ADR-B8 right to erasure; the
//! request is filed by the device, WP-B.6, or an org admin, `routes::privacy`). A phased
//! handler (G5): blob deletes happen with no transaction open, then one transaction deletes
//! rows, writes the tombstone and the audit row.
//!
//! ## What is erased
//!
//! - every session of the participant that no held clinical record documents, with its
//!   clips, review-queue items and labels (FK cascades), its idempotency key, its frames
//!   and report blobs (recorded key and the ADR-B3 key, since pre-signed uploads do not
//!   record one), and unsigned clinical review drafts about it;
//! - every care event, every draft referral letter;
//! - the artifact of every unexpired export that included the participant (EXJ-8); the
//!   export becomes `expired`, its pending deliveries fail (`export-unavailable`), and
//!   `export_items` stay (accounting of disclosures, EXJ-6). A case package (SHR-11) is
//!   found through its items and through `exports.case_participant_id`, so one whose
//!   participant has no item row still expires; every unrevoked share link of an export
//!   expired here is revoked (`revoked_by` NULL), audited `share_link.revoke` with
//!   `reason: erasure`. Downloads already made cannot be recalled;
//! - the MRN mapping (`patient_refs`), unless the participant is held;
//! - (WP-B.19, REC-7) every attachment with its encrypted blob, and every encounter,
//!   unless the participant is held (the hold rule as for notes).
//!
//! ## Clinical records: held, not erased (ADR-0008 Q2 is open)
//!
//! Signed clinical content is immutable by design (SIGN-7, NOTE-1, NOTE-3 triggers) and
//! medical-record retention rules may outlive an erasure request. Until the owner decides
//! ADR-0008 Q2, a participant with a signed or superseded review version, any clinical note,
//! an encounter with a clinician's summary (REC-3, the same kind of record as a note) or a
//! finalised letter is **held**: those records, the sessions they document, the MRN
//! mapping, the encounters and the attachments stay; everything else above is erased; the request ends `held`
//! (`hold_reason = clinical_record`, `completed_at` NULL, so a second request is refused as
//! a duplicate). Notes are not crypto-shredded: the DEK is per org (KEY-5), so shredding
//! one participant's notes is not possible without the per-record keys Q2 would decide.
//!
//! ## The tombstone
//!
//! A fully erased participant keeps its row: id, org, token, `withdrawn_at`, `erased_at`;
//! demographics and consent fields are cleared. The token stays so a replayed upload
//! (offline queue, retried batch) is still refused as `participant-withdrawn` instead of
//! re-creating the participant. The request row (`completed`, with counts) and the audit
//! row are the record that the erasure happened.
//!
//! ## Idempotency
//!
//! A request that is no longer `pending` returns its stored outcome. Blob deletes of absent
//! keys succeed, so a re-run after a crash between the phases repeats them harmlessly. The
//! record phase is fenced by `jobs.attempts` like the export job. A session that arrives
//! between the phases (an in-flight upload) makes the run retry rather than leave a blob
//! behind.
//!
//! ## Filing and replaying (WP-B.22)
//!
//! [`enqueue_erasure`] files a request for a participant (one open request per participant:
//! an open one is returned instead), withdraws consent and queues the job, in the caller's
//! transaction; `POST /v1/orgs/{id}/participants/{pid}/erasure` uses it. [`requeue`] is the
//! restore-time replay (`asd-backend admin requeue-erasure --ledger`, DPL-7): a participant
//! already erased is skipped; otherwise its open pending request gets a job (unless one is
//! queued or running) or a new request is filed, with audit `participant.erasure_requeued`
//! (`via: cli`). The job does the erasing, never the replay itself (trap 21).

use std::collections::BTreeSet;
use std::time::Duration;

use asd_contract::privacy::{ErasureOutcome, ErasureStatus, HOLD_CLINICAL_RECORD};
use serde_json::{json, Value};
use sqlx::PgConnection;
use uuid::Uuid;

use super::runner::JobError;
use super::{Job, JobContext, KIND_ERASURE};
use crate::audit::AuditEvent;
use crate::blob::{BlobError, BlobStore};
use crate::error::{AppError, AppResult};
use crate::ingest::idempotency;
use crate::routes::attachments;
use crate::routes::blobs::BlobKind;

/// Job params of an erasure.
pub fn params(request_id: Uuid) -> Value {
    json!({ "erasure_request_id": request_id })
}

/// Queues the erasure job of `request_id` in the caller's transaction and links it.
pub async fn enqueue(
    conn: &mut PgConnection,
    org: Uuid,
    request_id: Uuid,
    created_by: Option<Uuid>,
) -> AppResult<Uuid> {
    let job = super::enqueue(
        conn,
        KIND_ERASURE,
        Some(org),
        &params(request_id),
        created_by,
    )
    .await?;
    sqlx::query!(
        "UPDATE erasure_requests SET job_id = $2 WHERE id = $1",
        request_id,
        job.id
    )
    .execute(&mut *conn)
    .await?;
    Ok(job.id)
}

/// Who files an erasure request (`erasure_requests.requested_by_type`, `requested_by`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Actor {
    /// An org user through the console.
    User(Uuid),
    /// The service itself (restore replay); `requested_by` is NULL.
    System,
}

impl Actor {
    fn columns(self) -> (&'static str, Option<Uuid>) {
        match self {
            Self::User(id) => ("user", Some(id)),
            Self::System => ("system", None),
        }
    }
}

/// What [`enqueue_erasure`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Filed {
    /// A new request with its queued job.
    New { request_id: Uuid, job_id: Uuid },
    /// The participant already has an open request (pending or held); nothing changed.
    Open { request_id: Uuid },
}

/// Files an erasure request for `participant` of `org` in the caller's transaction: locks
/// the participant row (serialising concurrent requests), returns the open request if one
/// exists, otherwise inserts the request, withdraws consent now and queues the job. The
/// caller writes the audit row. `None` when the participant is not in `org`.
pub async fn enqueue_erasure(
    conn: &mut PgConnection,
    org: Uuid,
    participant: Uuid,
    requested_by: Actor,
    scope: &str,
) -> AppResult<Option<Filed>> {
    let found = sqlx::query_scalar!(
        "SELECT id FROM participants WHERE id = $1 AND org_id = $2 FOR UPDATE",
        participant,
        org,
    )
    .fetch_optional(&mut *conn)
    .await?;
    if found.is_none() {
        return Ok(None);
    }
    if let Some(request_id) = sqlx::query_scalar!(
        "SELECT id FROM erasure_requests WHERE participant_id = $1 AND completed_at IS NULL",
        participant,
    )
    .fetch_optional(&mut *conn)
    .await?
    {
        return Ok(Some(Filed::Open { request_id }));
    }
    let (by_type, by) = requested_by.columns();
    let request_id = sqlx::query_scalar!(
        "INSERT INTO erasure_requests (org_id, participant_id, requested_by_type, requested_by,
                                       scope)
         VALUES ($1, $2, $3, $4, $5) RETURNING id",
        org,
        participant,
        by_type,
        by,
        scope,
    )
    .fetch_one(&mut *conn)
    .await?;
    sqlx::query!(
        "UPDATE participants SET withdrawn_at = coalesce(withdrawn_at, now()) WHERE id = $1",
        participant,
    )
    .execute(&mut *conn)
    .await?;
    let created_by = match requested_by {
        Actor::User(id) => Some(id),
        Actor::System => None,
    };
    let job_id = enqueue(conn, org, request_id, created_by).await?;
    Ok(Some(Filed::New { request_id, job_id }))
}

/// What [`requeue`] did for one participant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Requeued {
    /// A job was queued (for the open pending request, or a new request); audited.
    Queued { request_id: Uuid, job_id: Uuid },
    /// `erased_at` is set: nothing to do.
    AlreadyErased,
    /// The open pending request has a queued or running job already.
    InProgress { request_id: Uuid },
    /// The open request is held (clinical record): the job would hold it again.
    Held { request_id: Uuid },
    /// No such participant in that org.
    Unknown,
}

/// Restore replay for one ledger entry (see the module docs), in its own transaction.
pub async fn requeue(pool: &sqlx::PgPool, org: Uuid, participant: Uuid) -> AppResult<Requeued> {
    let mut tx = pool.begin().await?;
    let Some(erased_at) = sqlx::query_scalar!(
        "SELECT erased_at FROM participants WHERE id = $1 AND org_id = $2 FOR UPDATE",
        participant,
        org,
    )
    .fetch_optional(&mut *tx)
    .await?
    else {
        return Ok(Requeued::Unknown);
    };
    if erased_at.is_some() {
        return Ok(Requeued::AlreadyErased);
    }
    let open = sqlx::query!(
        r#"SELECT r.id, r.status, j.status AS "job_status?"
           FROM erasure_requests r LEFT JOIN jobs j ON j.id = r.job_id
           WHERE r.participant_id = $1 AND r.completed_at IS NULL"#,
        participant,
    )
    .fetch_optional(&mut *tx)
    .await?;
    let (request_id, job_id, new_request) = match open {
        Some(r) if r.status == ErasureStatus::Held.as_str() => {
            return Ok(Requeued::Held { request_id: r.id });
        }
        Some(r) if matches!(r.job_status.as_deref(), Some("queued" | "running")) => {
            return Ok(Requeued::InProgress { request_id: r.id });
        }
        Some(r) => (r.id, enqueue(&mut tx, org, r.id, None).await?, false),
        None => match enqueue_erasure(&mut tx, org, participant, Actor::System, "all").await? {
            Some(Filed::New { request_id, job_id }) => (request_id, job_id, true),
            _ => {
                return Err(AppError::Internal(
                    "erasure request appeared while the participant was locked".into(),
                ))
            }
        },
    };
    AuditEvent::system("participant.erasure_requeued", "participant", participant)
        .org(org)
        .after(json!({
            "erasure_request_id": request_id,
            "job_id": job_id,
            "new_request": new_request,
            "via": "cli",
        }))
        .record(&mut tx)
        .await?;
    tx.commit().await?;
    Ok(Requeued::Queued { request_id, job_id })
}

fn permanent(m: impl Into<String>) -> JobError {
    JobError::Permanent(m.into())
}

pub(crate) fn blob_error(e: BlobError, attempts: i32) -> JobError {
    match e {
        BlobError::Unavailable(m) => JobError::RetryAfter(
            format!("blob-store-unavailable: {m}"),
            Duration::from_secs(60 * u64::from(attempts.unsigned_abs().max(1))),
        ),
        other => JobError::Transient(format!("blob-store: {other}")),
    }
}

/// Every key a session's frames or report blob may live under: the recorded one and the
/// ADR-B3 key (pre-signed uploads do not record theirs).
pub(crate) fn session_blob_keys(
    org: Uuid,
    token: &str,
    session_id: &str,
    frames_key: Option<&str>,
    report_key: Option<&str>,
    frames_only: bool,
) -> Vec<String> {
    let mut keys = BTreeSet::new();
    keys.insert(BlobKind::Frames.key(org, token, session_id));
    if let Some(k) = frames_key {
        keys.insert(k.to_owned());
    }
    if !frames_only {
        keys.insert(BlobKind::Report.key(org, token, session_id));
        if let Some(k) = report_key {
            keys.insert(k.to_owned());
        }
    }
    keys.into_iter().collect()
}

/// Deletes `keys`; a key the store refuses as invalid is skipped (it cannot exist there).
pub(crate) async fn delete_blobs(
    blobs: &dyn BlobStore,
    keys: &[String],
    attempts: i32,
) -> Result<u64, JobError> {
    let mut n = 0;
    for key in keys {
        match blobs.delete(key).await {
            Ok(()) => n += 1,
            Err(BlobError::InvalidKey) => {}
            Err(e) => return Err(blob_error(e, attempts)),
        }
    }
    Ok(n)
}

struct SessionRow {
    id: String,
    held: bool,
    keys: Vec<String>,
    idempotency_key: Option<String>,
}

/// Runs the erasure named in `job.params` (see the module docs).
pub async fn execute(ctx: &JobContext, job: &Job) -> Result<Value, JobError> {
    let request_id = job
        .params
        .get("erasure_request_id")
        .and_then(Value::as_str)
        .and_then(|s| Uuid::parse_str(s).ok())
        .ok_or_else(|| permanent("invalid erasure params: erasure_request_id"))?;
    let blobs = ctx
        .blobs
        .clone()
        .ok_or_else(|| permanent("this worker has no blob store"))?;
    let pool = &ctx.pool;

    let req = sqlx::query!(
        r#"SELECT r.org_id, r.participant_id, r.status, r.scope, r.outcome,
                  p.token, p.withdrawn_at
           FROM erasure_requests r JOIN participants p ON p.id = r.participant_id
           WHERE r.id = $1"#,
        request_id
    )
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| permanent("erasure request not found"))?;
    if req.status != ErasureStatus::Pending.as_str() {
        return Ok(
            json!({"erasure_request_id": request_id, "status": req.status,
                         "outcome": req.outcome}),
        );
    }
    if req.scope != "all" {
        return Err(permanent(format!(
            "unsupported erasure scope {:?}",
            req.scope
        )));
    }
    let (org, participant) = (req.org_id, req.participant_id);

    // An export that read its snapshot before the withdrawal may still include the
    // participant; wait for it, so its artifact is erased too (EXJ-8).
    let racing_export = sqlx::query_scalar!(
        r#"SELECT EXISTS (
               SELECT 1 FROM exports e JOIN jobs j ON j.id = e.job_id
               WHERE e.org_id = $1 AND e.status = 'running' AND j.status = 'running'
                 AND ($2::timestamptz IS NULL OR e.started_at <= $2)) AS "x!""#,
        org,
        req.withdrawn_at,
    )
    .fetch_one(pool)
    .await?;
    if racing_export {
        return Err(JobError::RetryAfter(
            "export-in-progress: waiting for a running export of the org".into(),
            Duration::from_secs(300),
        ));
    }

    // ---- phase 1: what to erase
    let sessions: Vec<SessionRow> = sqlx::query!(
        r#"SELECT s.id, s.participant_token, s.frames_blob_key, s.report_pdf_key,
                  s.idempotency_key,
                  (EXISTS (SELECT 1 FROM clinical_reviews cr
                           JOIN clinical_review_versions v ON v.review_id = cr.id
                           WHERE cr.session_id = s.id AND v.status <> 'draft')
                   OR EXISTS (SELECT 1 FROM clinical_notes n WHERE n.session_id = s.id)
                   OR EXISTS (SELECT 1 FROM referral_letters l
                              WHERE l.participant_id = s.participant_id
                                AND l.status = 'final' AND s.id = ANY(l.session_ids)))
                  AS "held!"
           FROM sessions s WHERE s.participant_id = $1 ORDER BY s.id"#,
        participant
    )
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|s| SessionRow {
        keys: session_blob_keys(
            org,
            &s.participant_token,
            &s.id,
            s.frames_blob_key.as_deref(),
            s.report_pdf_key.as_deref(),
            false,
        ),
        id: s.id,
        held: s.held,
        idempotency_key: s.idempotency_key,
    })
    .collect();
    let participant_held = sessions.iter().any(|s| s.held)
        || sqlx::query_scalar!(
            r#"SELECT (EXISTS (SELECT 1 FROM clinical_notes WHERE participant_id = $1)
                       OR EXISTS (SELECT 1 FROM encounters
                                  WHERE participant_id = $1 AND summary_ct IS NOT NULL)
                       OR EXISTS (SELECT 1 FROM referral_letters
                                  WHERE participant_id = $1 AND status = 'final')) AS "x!""#,
            participant
        )
        .fetch_one(pool)
        .await?;
    // EXJ-8 / SHR-11: through the items, and case packages through their participant.
    let exports = sqlx::query!(
        r#"SELECT e.id, e.blob_key FROM exports e
           WHERE e.status = 'succeeded'
             AND ((e.kind = 'bulk'
                   AND EXISTS (SELECT 1 FROM export_items i
                               WHERE i.export_id = e.id AND i.participant_id = $1))
                  OR e.case_participant_id = $1)
           ORDER BY e.id"#,
        participant
    )
    .fetch_all(pool)
    .await?;
    // REC-7: attachment blobs (deleted rows too; deleting an absent key succeeds). A row
    // without content may still get one before phase 3: its key is the computed one.
    let attachment_rows = sqlx::query!(
        "SELECT id, blob_key FROM attachments WHERE participant_id = $1 ORDER BY id",
        participant
    )
    .fetch_all(pool)
    .await?;
    let attachment_keys: Vec<String> = attachment_rows
        .iter()
        .map(|a| {
            a.blob_key
                .clone()
                .unwrap_or_else(|| attachments::blob_key(org, participant, a.id))
        })
        .collect();
    let attachments_with_content: BTreeSet<Uuid> = attachment_rows
        .iter()
        .filter(|a| a.blob_key.is_some())
        .map(|a| a.id)
        .collect();

    // ---- phase 2: blobs (no transaction open)
    let mut outcome = ErasureOutcome::default();
    for s in sessions.iter().filter(|s| !s.held) {
        outcome.session_blobs_deleted += delete_blobs(&*blobs, &s.keys, job.attempts).await?;
    }
    for e in &exports {
        if let Some(key) = &e.blob_key {
            delete_blobs(&*blobs, std::slice::from_ref(key), job.attempts).await?;
        }
    }
    if !participant_held {
        delete_blobs(&*blobs, &attachment_keys, job.attempts).await?;
    }

    // ---- phase 3: rows, tombstone, audit (one transaction)
    let erase: Vec<String> = sessions
        .iter()
        .filter(|s| !s.held)
        .map(|s| s.id.clone())
        .collect();
    let keys: Vec<String> = sessions
        .iter()
        .filter(|s| !s.held)
        .filter_map(|s| s.idempotency_key.clone())
        .collect();
    let export_ids: Vec<Uuid> = exports.iter().map(|e| e.id).collect();

    let mut tx = pool.begin().await?;
    let owns_job = sqlx::query_scalar!(
        r#"SELECT 1 AS "one!" FROM jobs
           WHERE id = $1 AND attempts = $2 AND status = 'running' FOR UPDATE"#,
        job.id,
        job.attempts,
    )
    .fetch_optional(&mut *tx)
    .await?
    .is_some();
    if !owns_job {
        tx.rollback().await?;
        return Err(permanent("job was reclaimed while running"));
    }
    let status = sqlx::query_scalar!(
        "SELECT status FROM erasure_requests WHERE id = $1 FOR UPDATE",
        request_id
    )
    .fetch_one(&mut *tx)
    .await?;
    if status != ErasureStatus::Pending.as_str() {
        tx.rollback().await?;
        return Err(permanent("erasure request is no longer pending"));
    }
    sqlx::query!(
        "SELECT id FROM participants WHERE id = $1 FOR UPDATE",
        participant
    )
    .fetch_one(&mut *tx)
    .await?;
    // A session that arrived after phase 1 has blobs nobody deleted: start over.
    let current: Vec<String> = sqlx::query_scalar!(
        "SELECT id FROM sessions WHERE participant_id = $1 ORDER BY id",
        participant
    )
    .fetch_all(&mut *tx)
    .await?;
    let known: BTreeSet<&str> = sessions.iter().map(|s| s.id.as_str()).collect();
    if current.iter().any(|id| !known.contains(id.as_str())) {
        tx.rollback().await?;
        return Err(JobError::Transient(
            "sessions changed during erasure; retrying".into(),
        ));
    }
    // Likewise an attachment whose content arrived after phase 2 (REC-7).
    if !participant_held {
        let stored: Vec<Uuid> = sqlx::query_scalar!(
            "SELECT id FROM attachments WHERE participant_id = $1 AND blob_key IS NOT NULL",
            participant
        )
        .fetch_all(&mut *tx)
        .await?;
        if stored
            .iter()
            .any(|id| !attachments_with_content.contains(id))
        {
            tx.rollback().await?;
            return Err(JobError::Transient(
                "attachments changed during erasure; retrying".into(),
            ));
        }
    }

    // EXJ-8: unexpired artifacts that included the participant.
    let expired: Vec<(Uuid, Uuid)> = sqlx::query!(
        "UPDATE exports SET status = 'expired', expired_at = now()
         WHERE id = ANY($1) AND status = 'succeeded'
         RETURNING id, org_id",
        &export_ids,
    )
    .fetch_all(&mut *tx)
    .await?
    .into_iter()
    .map(|r| (r.id, r.org_id))
    .collect();
    for (export_id, export_org) in &expired {
        AuditEvent::system("export.expire", "export", export_id)
            .org(*export_org)
            .after(json!({"reason": "erasure", "erasure_request_id": request_id}))
            .record(&mut tx)
            .await?;
    }
    outcome.export_artifacts_deleted = expired.len() as u64;
    // SHR-11: the share links of those exports stop answering (one 404, like any revoked
    // link). Links revoked earlier keep their own revocation.
    let expired_ids: Vec<Uuid> = expired.iter().map(|(id, _)| *id).collect();
    let revoked = sqlx::query!(
        "UPDATE share_links SET revoked_at = now()
         WHERE export_id = ANY($1) AND revoked_at IS NULL
         RETURNING id, org_id, export_id",
        &expired_ids,
    )
    .fetch_all(&mut *tx)
    .await?;
    for link in &revoked {
        AuditEvent::system("share_link.revoke", "share_link", link.id)
            .org(link.org_id)
            .after(json!({
                "export_id": link.export_id,
                "reason": "erasure",
                "erasure_request_id": request_id,
            }))
            .record(&mut tx)
            .await?;
    }

    // Unsigned clinical drafts about the erased sessions, then the reviews themselves
    // (a review with only drafts has no signatures and no auto-export).
    let review_drafts = sqlx::query!(
        "DELETE FROM clinical_review_versions v USING clinical_reviews cr
         WHERE v.review_id = cr.id AND cr.session_id = ANY($1) AND v.status = 'draft'",
        &erase,
    )
    .execute(&mut *tx)
    .await?
    .rows_affected();
    sqlx::query!(
        "DELETE FROM clinical_reviews WHERE session_id = ANY($1)",
        &erase
    )
    .execute(&mut *tx)
    .await?;
    let letter_drafts = sqlx::query!(
        "DELETE FROM referral_letters WHERE participant_id = $1 AND status = 'draft'",
        participant
    )
    .execute(&mut *tx)
    .await?
    .rows_affected();
    outcome.clinical_drafts_deleted = review_drafts + letter_drafts;

    sqlx::query!(
        "DELETE FROM idempotency_keys WHERE scope = $1 AND key = ANY($2)",
        idempotency::scope(org),
        &keys,
    )
    .execute(&mut *tx)
    .await?;
    // Clips, review-queue items and labels go with their session (ON DELETE CASCADE).
    outcome.sessions_deleted = sqlx::query!("DELETE FROM sessions WHERE id = ANY($1)", &erase)
        .execute(&mut *tx)
        .await?
        .rows_affected();
    outcome.sessions_held = (sessions.len() - erase.len()) as u64;
    outcome.care_events_deleted = sqlx::query!(
        "DELETE FROM care_events WHERE participant_id = $1",
        participant
    )
    .execute(&mut *tx)
    .await?
    .rows_affected();

    let final_status = if participant_held {
        // Held: no new data either (a system-filed request may not have withdrawn yet).
        sqlx::query!(
            "UPDATE participants SET withdrawn_at = coalesce(withdrawn_at, now()) WHERE id = $1",
            participant,
        )
        .execute(&mut *tx)
        .await?;
        ErasureStatus::Held
    } else {
        // REC-7: the attachments (their blobs went in phase 2), then the encounters; every
        // session is gone by now and a participant with notes is held, so nothing still
        // points at an encounter.
        outcome.attachments_deleted = sqlx::query!(
            "DELETE FROM attachments WHERE participant_id = $1",
            participant
        )
        .execute(&mut *tx)
        .await?
        .rows_affected();
        sqlx::query!(
            "DELETE FROM encounters WHERE participant_id = $1",
            participant
        )
        .execute(&mut *tx)
        .await?;
        outcome.patient_ref_deleted = sqlx::query!(
            "DELETE FROM patient_refs WHERE participant_id = $1",
            participant
        )
        .execute(&mut *tx)
        .await?
        .rows_affected()
            > 0;
        sqlx::query!(
            "UPDATE participants
             SET erased_at = now(), withdrawn_at = coalesce(withdrawn_at, now()),
                 age_bracket_at_enrol = NULL, language = NULL, consent_version = NULL,
                 consent_at = NULL, mode = NULL
             WHERE id = $1",
            participant
        )
        .execute(&mut *tx)
        .await?;
        ErasureStatus::Completed
    };
    let outcome_json = serde_json::to_value(&outcome).expect("plain data");
    sqlx::query!(
        "UPDATE erasure_requests
         SET status = $2, executed_at = now(),
             completed_at = CASE WHEN $2 = 'completed' THEN now() END,
             hold_reason = CASE WHEN $2 = 'held' THEN $3 END,
             outcome = $4
         WHERE id = $1",
        request_id,
        final_status.as_str(),
        HOLD_CLINICAL_RECORD,
        outcome_json,
    )
    .execute(&mut *tx)
    .await?;
    AuditEvent::system("participant.erasure_execute", "participant", participant)
        .org(org)
        .after(json!({
            "erasure_request_id": request_id,
            "status": final_status.as_str(),
            "hold_reason": participant_held.then_some(HOLD_CLINICAL_RECORD),
            "outcome": outcome_json,
        }))
        .record(&mut tx)
        .await?;
    tx.commit().await?;

    // DLV-1: deliveries of an artifact that no longer exists fail with it.
    for (export_id, _) in &expired {
        if let Err(e) = crate::interop::outbox::fail_for_export(
            pool,
            *export_id,
            crate::interop::ErrorClass::ExportUnavailable,
        )
        .await
        {
            tracing::warn!(%export_id, error = %e, "could not fail an erased export's deliveries");
        }
    }
    Ok(json!({
        "erasure_request_id": request_id,
        "status": final_status.as_str(),
        "outcome": outcome_json,
    }))
}

/// Queues erasure jobs for pending requests that have none running (requests filed before
/// this package, or whose job gave up more than an hour ago). Returns how many were queued.
pub async fn schedule_pending(pool: &sqlx::PgPool) -> AppResult<u64> {
    let mut tx = pool.begin().await?;
    let due = sqlx::query!(
        r#"SELECT r.id, r.org_id FROM erasure_requests r
           LEFT JOIN jobs j ON j.id = r.job_id
           WHERE r.status = 'pending'
             AND (j.id IS NULL
                  OR (j.status IN ('failed', 'succeeded', 'cancelled')
                      AND coalesce(j.finished_at, j.created_at) < now() - interval '1 hour'))
           ORDER BY r.requested_at
           LIMIT 100
           FOR UPDATE OF r SKIP LOCKED"#
    )
    .fetch_all(&mut *tx)
    .await?;
    for r in &due {
        enqueue(&mut tx, r.org_id, r.id, None).await?;
    }
    tx.commit().await?;
    Ok(due.len() as u64)
}
