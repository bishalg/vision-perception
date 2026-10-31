//! owner: WP-C.10
//!
//! Sign, cosign and amend (SIGN-4..10).
//!
//! - **Sign** binds the signature to `content_hash = SHA-256(JCS(snapshot))` recomputed on
//!   the server from the stored session data and the decrypted draft; the client must echo
//!   the hash it displayed (`409 content-changed` otherwise). Step-up re-authentication in
//!   the same request (SIGN-5); a failure is audited (`clinical_review.sign_failed`) in its
//!   own transaction and changes nothing. The org's cosign policy decides `final` or
//!   `awaiting_cosign` (SIGN-6); signing an amendment supersedes the earlier signed version
//!   (SIGN-8).
//! - **Cosign** signs the same signed version, by a different user (`409
//!   cosigner-must-differ`, also enforced by the 0008 CHECK and trigger).
//! - **Amend** copies the latest signed content into a new draft (`version_no + 1`),
//!   re-encrypted under the new row id (AAD), with the encrypted reason; while
//!   `awaiting_cosign` this withdraws the pending cosign (audited in the same row).
//!
//! Reaching `final` calls [`crate::interop::on_review_finalized`] in the same transaction
//! (SIGN-10). Every mutation writes exactly one audit row with `content_hash` and
//! `version_no`, never the text (SIGN-9).

use asd_contract::clinical::{
    AmendClinicalReviewRequest, ClinicalReviewState, ReviewVersionStatus, SignRequest,
    SignatureRole, MAX_AMEND_REASON_CHARS, MIN_AMEND_REASON_CHARS, SIGNATURE_MEANING,
};
use asd_contract::problem::{
    CONFLICT, CONTENT_CHANGED, COSIGNER_MUST_DIFFER, SIGNER_PROFILE_INCOMPLETE,
};
use serde_json::json;
use uuid::Uuid;

use super::reauth::{check_sign_body, step_up, StepUpError, StepUpLimiter};
use super::review::{
    content_hash, current_version, decrypt_content, find_review, hex, latest_signed_version,
    lock_review, seal, session_facts, transition, ReviewOp, ReviewRow,
};
use super::{cosign_required, ClinicalSettings};
use crate::app::AppState;
use crate::audit::AuditEvent;
use crate::auth::extractors::{RequestMeta, UserPrincipal};
use crate::crypto::ClinicalCrypto;
use crate::error::{AppError, AppResult};
use crate::interop;
use crate::problem::InvalidParam;

fn content_changed() -> AppError {
    AppError::Conflict {
        slug: CONTENT_CHANGED,
        detail: "the review content changed since it was displayed; reload and review again".into(),
    }
}

fn changed_since_read() -> AppError {
    AppError::Conflict {
        slug: CONFLICT,
        detail: "the review changed during this request; reload it".into(),
    }
}

/// The signer's name and registration (NOTE-6); `409 signer-profile-incomplete` without a
/// display name.
async fn signer_profile(state: &AppState, user: Uuid) -> AppResult<(String, Option<String>)> {
    let row = sqlx::query!(
        "SELECT display_name, professional_registration FROM users WHERE id = $1",
        user
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::Unauthorized)?;
    match row.display_name {
        Some(name) if !name.trim().is_empty() => Ok((name, row.professional_registration)),
        _ => Err(AppError::Conflict {
            slug: SIGNER_PROFILE_INCOMPLETE,
            detail: "set your display name in your profile before signing".into(),
        }),
    }
}

/// Loads the review for `principal` (404 for another org's).
async fn review_for(state: &AppState, principal: &UserPrincipal, id: Uuid) -> AppResult<ReviewRow> {
    let review = find_review(&state.db, id)
        .await?
        .ok_or(AppError::NotFound)?;
    principal.ensure_org(review.org_id)?;
    Ok(review)
}

/// Step-up; a failed check writes `action` (`clinical_review.sign_failed` /
/// `.cosign_failed`) in its own transaction, then answers 401.
async fn step_up_audited(
    state: &AppState,
    limiter: &StepUpLimiter,
    principal: &UserPrincipal,
    meta: &RequestMeta,
    review: &ReviewRow,
    req: &SignRequest,
    action: &str,
) -> AppResult<()> {
    match step_up(
        &state.db,
        limiter,
        principal.user_id,
        &req.password,
        req.totp_code.as_deref(),
    )
    .await
    {
        Ok(()) => Ok(()),
        Err(StepUpError::Failed) => {
            let mut tx = state.db.begin().await?;
            AuditEvent::user(principal, action, "clinical_review", review.id)
                .org(review.org_id)
                .after(json!({"state": review.state.as_str(), "reason": "reauthentication-failed"}))
                .meta(meta)
                .record(&mut tx)
                .await?;
            tx.commit().await?;
            Err(StepUpError::Failed.into())
        }
        Err(e) => Err(e.into()),
    }
}

/// `POST /v1/clinical-reviews/{id}/sign`.
pub async fn sign(
    state: &AppState,
    crypto: &ClinicalCrypto,
    limiter: &StepUpLimiter,
    principal: &UserPrincipal,
    meta: &RequestMeta,
    review_id: Uuid,
    req: SignRequest,
) -> AppResult<()> {
    check_sign_body(&req.content_hash, req.meaning_ack)?;
    let review = review_for(state, principal, review_id).await?;
    let org = review.org_id;
    let (display_name, registration) = signer_profile(state, principal.user_id).await?;
    step_up_audited(
        state,
        limiter,
        principal,
        meta,
        &review,
        &req,
        "clinical_review.sign_failed",
    )
    .await?;

    // Before the transaction: the state allows signing, the draft is complete, and the
    // hash of what is stored equals the hash the signer saw.
    transition(
        review.state,
        ReviewOp::Sign {
            cosign_required: false,
        },
    )?;
    let version = current_version(&state.db, review_id).await?;
    if version.status != ReviewVersionStatus::Draft {
        return Err(changed_since_read());
    }
    let (content, _) = decrypt_content(crypto, org, &version).await?;
    if content.disposition.is_none() {
        return Err(AppError::Validation(vec![InvalidParam {
            name: "disposition".into(),
            reason: "is required to sign; save a disposition first".into(),
        }]));
    }
    let (facts, settings) = {
        let mut conn = state.db.acquire().await?;
        let facts = session_facts(&mut conn, &review.session_id)
            .await?
            .ok_or(AppError::NotFound)?;
        (facts, ClinicalSettings::load(&mut conn, org).await?)
    };
    let hash = content_hash(&facts, version.version_no, &content);
    if hex(&hash) != req.content_hash.to_ascii_lowercase() {
        return Err(content_changed());
    }
    let cosign = cosign_required(settings.cosign_policy, &facts.attention_flag);

    let mut tx = state.db.begin().await?;
    let locked = lock_review(&mut tx, review_id).await?;
    if locked.row_version != review.row_version {
        // Someone saved, signed or amended since the hash was checked.
        return Err(content_changed());
    }
    let next = transition(
        locked.state,
        ReviewOp::Sign {
            cosign_required: cosign,
        },
    )?;
    sqlx::query!(
        r#"UPDATE clinical_review_versions
           SET status = 'signed', content_hash = $2, signed_at = now(), updated_by = $3,
               updated_at = now()
           WHERE id = $1 AND status = 'draft'"#,
        version.id,
        hash.as_slice(),
        principal.user_id,
    )
    .execute(&mut *tx)
    .await?;
    // SIGN-8: the earlier signed version is superseded only now that the amendment is signed.
    sqlx::query!(
        r#"UPDATE clinical_review_versions SET status = 'superseded', superseded_at = now()
           WHERE review_id = $1 AND status = 'signed' AND version_no < $2"#,
        review_id,
        version.version_no,
    )
    .execute(&mut *tx)
    .await?;
    sqlx::query!(
        r#"INSERT INTO clinical_signatures
               (org_id, review_id, version_id, signer_id, role, meaning, content_hash,
                signer_display_name, signer_registration)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)"#,
        org,
        review_id,
        version.id,
        principal.user_id,
        SignatureRole::Sign.as_str(),
        SIGNATURE_MEANING,
        hash.as_slice(),
        display_name,
        registration,
    )
    .execute(&mut *tx)
    .await?;
    sqlx::query!(
        r#"UPDATE clinical_reviews SET state = $2, cosign_required = $3,
               row_version = row_version + 1, updated_at = now()
           WHERE id = $1"#,
        review_id,
        next.as_str(),
        cosign,
    )
    .execute(&mut *tx)
    .await?;
    AuditEvent::user(
        principal,
        "clinical_review.sign",
        "clinical_review",
        review_id,
    )
    .org(org)
    .before(json!({"state": locked.state.as_str()}))
    .after(json!({
        "state": next.as_str(),
        "version_no": version.version_no,
        "content_hash": hex(&hash),
        "cosign_required": cosign,
    }))
    .meta(meta)
    .record(&mut tx)
    .await?;
    if next == ClinicalReviewState::Final {
        interop::on_review_finalized(&mut tx, review_id).await?;
    }
    tx.commit().await?;
    Ok(())
}

/// `POST /v1/clinical-reviews/{id}/cosign`.
pub async fn cosign(
    state: &AppState,
    crypto: &ClinicalCrypto,
    limiter: &StepUpLimiter,
    principal: &UserPrincipal,
    meta: &RequestMeta,
    review_id: Uuid,
    req: SignRequest,
) -> AppResult<()> {
    check_sign_body(&req.content_hash, req.meaning_ack)?;
    let review = review_for(state, principal, review_id).await?;
    let org = review.org_id;
    transition(review.state, ReviewOp::Cosign)?;
    let version = latest_signed_version(&state.db, review_id)
        .await?
        .ok_or_else(|| AppError::Internal("review awaiting cosign has no signed version".into()))?;
    let primary = sqlx::query_scalar!(
        "SELECT signer_id FROM clinical_signatures WHERE version_id = $1 AND role = 'sign'",
        version.id
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::Internal("signed version has no primary signature".into()))?;
    if primary == principal.user_id {
        return Err(AppError::Conflict {
            slug: COSIGNER_MUST_DIFFER,
            detail: "the second signature must come from a different clinician".into(),
        });
    }
    let (display_name, registration) = signer_profile(state, principal.user_id).await?;
    step_up_audited(
        state,
        limiter,
        principal,
        meta,
        &review,
        &req,
        "clinical_review.cosign_failed",
    )
    .await?;

    // The cosigner saw the signed content: its stored hash, which must still match the
    // stored session data and content (trap 8).
    let stored = version
        .content_hash
        .clone()
        .ok_or_else(|| AppError::Internal("signed version has no content hash".into()))?;
    if hex(&stored) != req.content_hash.to_ascii_lowercase() {
        return Err(content_changed());
    }
    let (content, _) = decrypt_content(crypto, org, &version).await?;
    let facts = {
        let mut conn = state.db.acquire().await?;
        session_facts(&mut conn, &review.session_id)
            .await?
            .ok_or(AppError::NotFound)?
    };
    if content_hash(&facts, version.version_no, &content).as_slice() != stored.as_slice() {
        return Err(content_changed());
    }

    let mut tx = state.db.begin().await?;
    let locked = lock_review(&mut tx, review_id).await?;
    if locked.row_version != review.row_version {
        return Err(changed_since_read());
    }
    let next = transition(locked.state, ReviewOp::Cosign)?;
    sqlx::query!(
        r#"INSERT INTO clinical_signatures
               (org_id, review_id, version_id, signer_id, role, meaning, content_hash,
                signer_display_name, signer_registration, primary_signer_id)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)"#,
        org,
        review_id,
        version.id,
        principal.user_id,
        SignatureRole::Cosign.as_str(),
        SIGNATURE_MEANING,
        stored.as_slice(),
        display_name,
        registration,
        primary,
    )
    .execute(&mut *tx)
    .await?;
    sqlx::query!(
        r#"UPDATE clinical_reviews SET state = $2, row_version = row_version + 1,
               updated_at = now()
           WHERE id = $1"#,
        review_id,
        next.as_str(),
    )
    .execute(&mut *tx)
    .await?;
    AuditEvent::user(
        principal,
        "clinical_review.cosign",
        "clinical_review",
        review_id,
    )
    .org(org)
    .before(json!({"state": locked.state.as_str()}))
    .after(json!({
        "state": next.as_str(),
        "version_no": version.version_no,
        "content_hash": hex(&stored),
        "primary_signer_id": primary,
    }))
    .meta(meta)
    .record(&mut tx)
    .await?;
    interop::on_review_finalized(&mut tx, review_id).await?;
    tx.commit().await?;
    Ok(())
}

/// SIGN-8 reason: 10..500 characters (after trimming for the lower bound).
fn check_reason(reason: &str) -> AppResult<()> {
    let n = reason.chars().count();
    if reason.trim().chars().count() < MIN_AMEND_REASON_CHARS || n > MAX_AMEND_REASON_CHARS {
        return Err(AppError::Validation(vec![InvalidParam {
            name: "reason".into(),
            reason: format!(
                "must be {MIN_AMEND_REASON_CHARS} to {MAX_AMEND_REASON_CHARS} characters"
            ),
        }]));
    }
    Ok(())
}

/// `POST /v1/clinical-reviews/{id}/amend`.
pub async fn amend(
    state: &AppState,
    crypto: &ClinicalCrypto,
    principal: &UserPrincipal,
    meta: &RequestMeta,
    review_id: Uuid,
    req: AmendClinicalReviewRequest,
) -> AppResult<()> {
    check_reason(&req.reason)?;
    let review = review_for(state, principal, review_id).await?;
    let org = review.org_id;
    transition(review.state, ReviewOp::Amend)?;
    let signed = latest_signed_version(&state.db, review_id)
        .await?
        .ok_or_else(|| AppError::Internal("signed review has no signed version".into()))?;
    let latest = current_version(&state.db, review_id).await?;
    let version_no = latest.version_no + 1;

    // The copied content is re-encrypted: the AAD names the new row (trap 9).
    let (content, _) = decrypt_content(crypto, org, &signed).await?;
    let new_id = Uuid::new_v4();
    let impression = seal(
        crypto,
        org,
        new_id,
        "impression_ct",
        content.impression.as_deref(),
    )
    .await?;
    let recommendations = seal(
        crypto,
        org,
        new_id,
        "recommendations_ct",
        content.recommendations.as_deref(),
    )
    .await?;
    let reason = seal(crypto, org, new_id, "amend_reason_ct", Some(&req.reason)).await?;
    let facts = {
        let mut conn = state.db.acquire().await?;
        session_facts(&mut conn, &review.session_id)
            .await?
            .ok_or(AppError::NotFound)?
    };
    let hash = content_hash(&facts, version_no, &content);

    let mut tx = state.db.begin().await?;
    let locked = lock_review(&mut tx, review_id).await?;
    if locked.row_version != review.row_version {
        return Err(changed_since_read());
    }
    let next = transition(locked.state, ReviewOp::Amend)?;
    let withdrew_cosign = locked.state == ClinicalReviewState::AwaitingCosign;
    sqlx::query!(
        r#"INSERT INTO clinical_review_versions
               (id, review_id, org_id, version_no, status, disposition, impression_ct,
                impression_key_id, recommendations_ct, recommendations_key_id,
                amend_reason_ct, amend_reason_key_id, content_hash, created_by, updated_by)
           VALUES ($1, $2, $3, $4, 'draft', $5, $6, $7, $8, $9, $10, $11, $12, $13, $13)"#,
        new_id,
        review_id,
        org,
        version_no,
        content.disposition.map(|d| d.as_str()),
        impression.0,
        impression.1,
        recommendations.0,
        recommendations.1,
        reason.0,
        reason.1,
        hash.as_slice(),
        principal.user_id,
    )
    .execute(&mut *tx)
    .await?;
    sqlx::query!(
        r#"UPDATE clinical_reviews SET state = $2, row_version = row_version + 1,
               updated_at = now()
           WHERE id = $1"#,
        review_id,
        next.as_str(),
    )
    .execute(&mut *tx)
    .await?;
    AuditEvent::user(
        principal,
        "clinical_review.amend",
        "clinical_review",
        review_id,
    )
    .org(org)
    .before(json!({"state": locked.state.as_str(), "version_no": signed.version_no}))
    .after(json!({
        "state": next.as_str(),
        "version_no": version_no,
        "content_hash": hex(&hash),
        "withdrew_cosign": withdrew_cosign,
    }))
    .meta(meta)
    .record(&mut tx)
    .await?;
    tx.commit().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reason_bounds() {
        assert!(check_reason("too short").is_err());
        assert!(
            check_reason("          x").is_err(),
            "whitespace does not count"
        );
        assert!(check_reason("long enough").is_ok());
        assert!(check_reason(&"é".repeat(500)).is_ok());
        assert!(check_reason(&"é".repeat(501)).is_err());
    }
}
