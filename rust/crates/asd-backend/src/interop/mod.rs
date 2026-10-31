//! owner: WP-C.10 (stub); WP-I.7 (delivery channels, outbox)
//!
//! Delivery of exports to hospital systems (INTG-1..5, DLV-1..6, ADR-0008 D11).
//!
//! - [`destination`]: `integration_destinations` rows, per-kind config rules (INTG-3) and
//!   the write-only secret (INTG-2: encrypted with the org data key, AAD over the row).
//! - [`netguard`]: the outbound SSRF guard (DLV-3): resolve once, check, connect to the
//!   checked address.
//! - [`webhook`]: Standard Webhooks signed `POST` with an event and a short-lived download
//!   link only (DLV-2).
//! - [`sftp`] and [`ssh_keys`]: SFTP drop with a server-generated Ed25519 key and a pinned
//!   host key (DLV-4, INTG-5).
//! - [`outbox`]: delivery rows and jobs (DLV-1), and SIGN-10's [`on_review_finalized`]
//!   (DLV-6).
//!
//! Every failure is reported as an [`ErrorClass`], a fixed spelling stored in
//! `export_deliveries.last_error_class`, `jobs.error` and `last_test_error`; remote response
//! bodies, URLs with queries and SSH banners are never stored or logged (trap 20).

pub mod destination;
pub mod netguard;
pub mod outbox;
pub mod sftp;
pub mod ssh_keys;
pub mod webhook;

use std::sync::OnceLock;

use sqlx::PgConnection;
use uuid::Uuid;

use crate::error::AppResult;

/// Why a delivery or a connection test failed. Only these spellings are ever stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorClass {
    /// The destination's config is incomplete or invalid for its kind.
    InvalidConfig,
    /// No secret (webhook) or no SSH key (SFTP) is set.
    MissingSecret,
    /// The name did not resolve.
    DnsFailed,
    /// The name resolves to a refused address (DLV-3).
    AddressBlocked,
    ConnectFailed,
    Timeout,
    TlsFailed,
    /// The receiver answered a redirect; it is not followed (DLV-3).
    HttpRedirect,
    /// The receiver answered a non-2xx status (`remote_ref` holds it).
    HttpStatus,
    /// INTG-5: the SFTP server's key is not the pinned one.
    HostKeyMismatch,
    /// The SFTP server refused our key.
    AuthFailed,
    /// An SFTP operation (open, write, rename) failed.
    SftpFailed,
    /// SSH protocol failure before authentication.
    SshFailed,
    /// The export artifact is gone (expired) or never succeeded.
    ExportUnavailable,
    /// No absolute download link can be made (proxy blob mode without
    /// `ASD_PUBLIC_BASE_URL`).
    DownloadUrlUnavailable,
    BlobUnavailable,
    /// The database failed while preparing the attempt.
    DatabaseUnavailable,
    /// The worker has no `ASD_DATA_KEK`, so the secret cannot be opened.
    CryptoUnavailable,
    /// The channel for this kind does not exist yet (`fhir_server`, `hl7_mllp`).
    UnsupportedKind,
    /// The destination was disabled after the delivery was queued.
    DestinationDisabled,
}

impl ErrorClass {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidConfig => "invalid-config",
            Self::MissingSecret => "missing-secret",
            Self::DnsFailed => "dns-failed",
            Self::AddressBlocked => "address-blocked",
            Self::ConnectFailed => "connect-failed",
            Self::Timeout => "timeout",
            Self::TlsFailed => "tls-failed",
            Self::HttpRedirect => "http-redirect",
            Self::HttpStatus => "http-status",
            Self::HostKeyMismatch => asd_contract::problem::HOST_KEY_MISMATCH,
            Self::AuthFailed => "auth-failed",
            Self::SftpFailed => "sftp-failed",
            Self::SshFailed => "ssh-failed",
            Self::ExportUnavailable => "export-unavailable",
            Self::DownloadUrlUnavailable => "download-url-unavailable",
            Self::BlobUnavailable => "blob-unavailable",
            Self::DatabaseUnavailable => "database-unavailable",
            Self::CryptoUnavailable => "clinical-crypto-unavailable",
            Self::UnsupportedKind => "unsupported-kind",
            Self::DestinationDisabled => "destination-disabled",
        }
    }

    /// Whether another attempt could succeed without someone changing the destination.
    /// A host-key mismatch never retries into trust (INTG-5); an address the guard refuses
    /// stays refused.
    pub const fn is_retryable(self) -> bool {
        matches!(
            self,
            Self::DnsFailed
                | Self::ConnectFailed
                | Self::Timeout
                | Self::TlsFailed
                | Self::HttpStatus
                | Self::SftpFailed
                | Self::SshFailed
                | Self::BlobUnavailable
                | Self::DatabaseUnavailable
        )
    }
}

impl std::fmt::Display for ErrorClass {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A failed attempt: the class and, when there is one, a short remote reference such as
/// `http 503` (never a body).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeliveryFailure {
    pub class: ErrorClass,
    pub remote_ref: Option<String>,
}

impl From<ErrorClass> for DeliveryFailure {
    fn from(class: ErrorClass) -> Self {
        Self {
            class,
            remote_ref: None,
        }
    }
}

/// Test double: integration tests count calls per review through an observer (a plain
/// `fn`, so production keeps no state). Unset outside tests.
static FINALIZE_OBSERVER: OnceLock<fn(Uuid)> = OnceLock::new();

/// Installs the observer once per process; later calls are ignored.
pub fn set_finalize_observer(observer: fn(Uuid)) {
    let _ = FINALIZE_OBSERVER.set(observer);
}

/// SIGN-10 hook, called inside the transaction that makes `review_id` final, after the
/// review row and its signature are written and before the commit.
///
/// DLV-6 (transactional outbox): creates one identified `clinical_care` export of the
/// review's session and one delivery per active `production` destination with
/// `trigger = on_final_sign`, on this transaction, so they exist if and only if the
/// signature commits. The work runs inside a savepoint and never fails the caller: an
/// error is rolled back to the savepoint and logged, and the signature commits without
/// auto-deliveries (a failed delivery never blocks or undoes a signature).
pub async fn on_review_finalized(tx: &mut PgConnection, review_id: Uuid) -> AppResult<()> {
    outbox::enqueue_auto_deliveries(tx, review_id).await;
    if let Some(observer) = FINALIZE_OBSERVER.get() {
        observer(review_id);
    }
    Ok(())
}
