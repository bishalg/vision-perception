//! owner: WP-C.10 (stub); WP-I.7 fills it
//!
//! Interop hooks of the clinician workspace. SIGN-10: when a clinical review reaches
//! `final` (a first signature without cosign, a cosign, or a signed amendment), the signing
//! transaction calls [`on_review_finalized`] exactly once. WP-C.10 ships it as a no-op;
//! WP-I.7 turns it into the transactional outbox (DLV-6: enqueue the auto-deliveries in the
//! same transaction, so a delivery exists if and only if the signature committed, and a
//! failed delivery never undoes the signature).

use std::sync::OnceLock;

use sqlx::PgConnection;
use uuid::Uuid;

use crate::error::AppResult;

/// Test double: integration tests count calls per review through an observer (a plain
/// `fn`, so production keeps no state). Unset outside tests.
static FINALIZE_OBSERVER: OnceLock<fn(Uuid)> = OnceLock::new();

/// Installs the observer once per process; later calls are ignored.
pub fn set_finalize_observer(observer: fn(Uuid)) {
    let _ = FINALIZE_OBSERVER.set(observer);
}

/// SIGN-10 hook, called inside the transaction that makes `review_id` final, after the
/// review row and its signature are written and before the commit. No-op in WP-C.10.
pub async fn on_review_finalized(tx: &mut PgConnection, review_id: Uuid) -> AppResult<()> {
    let _ = tx; // WP-I.7: INSERT the export and delivery rows on this transaction.
    if let Some(observer) = FINALIZE_OBSERVER.get() {
        observer(review_id);
    }
    Ok(())
}
