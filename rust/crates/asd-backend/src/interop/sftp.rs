//! owner: WP-I.7
//!
//! SFTP drop (DLV-4, INTG-5) over `russh` / `russh-sftp` (pure Rust, no libssh2/OpenSSL).
//!
//! 1. Resolve and check the host ([`netguard::resolve`], SFTP rules), connect to a checked
//!    address.
//! 2. The server's host key must match the pinned `host_key_sha256`; otherwise the
//!    handshake is aborted before authentication and nothing is uploaded
//!    (`host-key-mismatch`, never trust-on-first-use). Host certificates are refused.
//! 3. Authenticate with the destination's Ed25519 key (no passwords).
//! 4. Write `{name}.sha256` (`sha256sum` format), then the artifact, each to
//!    `{dir}/.{file}.part` first and renamed into place, so a receiver never picks up a
//!    partial file and the checksum is there when the artifact appears. A target left by an
//!    earlier attempt is replaced (re-runs are idempotent).

use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use asd_contract::export::IntegrationConfig;
use russh::client;
use russh::keys::{PrivateKey, PrivateKeyWithHashAlg, PublicKeyOrCertificate};
use russh_sftp::client::SftpSession;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;

use super::netguard::{self, Channel};
use super::{ssh_keys, DeliveryFailure, ErrorClass};

pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// Handshake and authentication together.
pub const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(30);
/// A session silent for this long is dropped.
pub const INACTIVITY_TIMEOUT: Duration = Duration::from_secs(60);

/// Where and how to drop files (INTG-3 `sftp {...}`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SftpTarget {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub remote_dir: String,
    /// Normalised base64 of the pinned SHA-256 host key fingerprint.
    pub host_key_sha256: String,
    pub file_prefix: String,
}

impl SftpTarget {
    pub fn from_config(c: &IntegrationConfig) -> Result<Self, ErrorClass> {
        let host = c
            .host
            .as_deref()
            .map(str::trim)
            .filter(|h| !h.is_empty())
            .ok_or(ErrorClass::InvalidConfig)?;
        let pinned = c
            .host_key_sha256
            .as_deref()
            .and_then(ssh_keys::normalise_fingerprint)
            .ok_or(ErrorClass::InvalidConfig)?;
        Ok(Self {
            host: host.to_owned(),
            port: c.port.filter(|p| *p != 0).unwrap_or(22),
            username: c.username.clone().ok_or(ErrorClass::InvalidConfig)?,
            remote_dir: c
                .remote_dir
                .clone()
                .ok_or(ErrorClass::InvalidConfig)?
                .trim_end_matches('/')
                .to_owned(),
            host_key_sha256: pinned,
            file_prefix: c.file_prefix.clone().unwrap_or_default(),
        })
    }

    fn path(&self, file: &str) -> String {
        format!("{}/{file}", self.remote_dir)
    }
}

/// Client handler: accepts exactly the pinned host key and records a mismatch.
struct PinnedHostKey {
    pinned: String,
    mismatch: Arc<AtomicBool>,
}

impl client::Handler for PinnedHostKey {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        server_key: &PublicKeyOrCertificate,
    ) -> Result<bool, Self::Error> {
        let ok = match server_key {
            PublicKeyOrCertificate::PublicKey { key, .. } => {
                ssh_keys::host_key_matches(key, &self.pinned)
            }
            PublicKeyOrCertificate::Certificate(_) => false,
        };
        if !ok {
            self.mismatch.store(true, Ordering::SeqCst);
        }
        Ok(ok)
    }
}

async fn connect_tcp(addrs: &[SocketAddr]) -> Result<TcpStream, ErrorClass> {
    let mut last = ErrorClass::ConnectFailed;
    for addr in addrs {
        match tokio::time::timeout(CONNECT_TIMEOUT, TcpStream::connect(addr)).await {
            Ok(Ok(stream)) => {
                let _ = stream.set_nodelay(true);
                return Ok(stream);
            }
            Ok(Err(_)) => last = ErrorClass::ConnectFailed,
            Err(_) => last = ErrorClass::Timeout,
        }
    }
    Err(last)
}

fn sftp_failed(_: russh_sftp::client::error::Error) -> DeliveryFailure {
    ErrorClass::SftpFailed.into()
}

async fn put(
    sftp: &SftpSession,
    target: &SftpTarget,
    file: &str,
    bytes: &[u8],
) -> Result<(), DeliveryFailure> {
    let part = target.path(&format!(".{file}.part"));
    let fin = target.path(file);
    let mut f = sftp.create(part.clone()).await.map_err(sftp_failed)?;
    f.write_all(bytes)
        .await
        .map_err(|_| DeliveryFailure::from(ErrorClass::SftpFailed))?;
    f.shutdown()
        .await
        .map_err(|_| DeliveryFailure::from(ErrorClass::SftpFailed))?;
    // SFTP v3 rename refuses an existing target: replace one left by an earlier attempt.
    let _ = sftp.remove_file(fin.clone()).await;
    sftp.rename(part, fin).await.map_err(sftp_failed)?;
    Ok(())
}

/// Drops `bytes` as `{prefix}{name}` (and its `.sha256`) on the target. `Ok` is the remote
/// reference (`sftp:{dir}/{file}`).
pub async fn drop_file(
    target: &SftpTarget,
    key: PrivateKey,
    name: &str,
    bytes: &[u8],
    sha256_hex: &str,
) -> Result<String, DeliveryFailure> {
    let addrs = netguard::resolve(&target.host, target.port, Channel::Sftp).await?;
    let stream = connect_tcp(&addrs).await?;
    let mismatch = Arc::new(AtomicBool::new(false));
    let handler = PinnedHostKey {
        pinned: target.host_key_sha256.clone(),
        mismatch: mismatch.clone(),
    };
    let config = Arc::new(client::Config {
        inactivity_timeout: Some(INACTIVITY_TIMEOUT),
        ..Default::default()
    });
    let session = async {
        let mut handle = client::connect_stream(config, stream, handler)
            .await
            .map_err(|e| match e {
                _ if mismatch.load(Ordering::SeqCst) => ErrorClass::HostKeyMismatch,
                russh::Error::IO(_) => ErrorClass::ConnectFailed,
                _ => ErrorClass::SshFailed,
            })?;
        let auth = handle
            .authenticate_publickey(
                target.username.clone(),
                PrivateKeyWithHashAlg::new(Arc::new(key), None),
            )
            .await
            .map_err(|_| ErrorClass::SshFailed)?;
        if !auth.success() {
            return Err(ErrorClass::AuthFailed);
        }
        Ok(handle)
    };
    let handle = tokio::time::timeout(HANDSHAKE_TIMEOUT, session)
        .await
        .map_err(|_| {
            if mismatch.load(Ordering::SeqCst) {
                ErrorClass::HostKeyMismatch
            } else {
                ErrorClass::Timeout
            }
        })??;

    let channel = handle
        .channel_open_session()
        .await
        .map_err(|_| ErrorClass::SftpFailed)?;
    channel
        .request_subsystem(true, "sftp")
        .await
        .map_err(|_| ErrorClass::SftpFailed)?;
    let sftp = SftpSession::new(channel.into_stream())
        .await
        .map_err(sftp_failed)?;

    let file = format!("{}{name}", target.file_prefix);
    let checksum = format!("{sha256_hex}  {file}\n");
    put(
        &sftp,
        target,
        &format!("{file}.sha256"),
        checksum.as_bytes(),
    )
    .await?;
    put(&sftp, target, &file, bytes).await?;
    let _ = sftp.close().await;
    let _ = handle
        .disconnect(russh::Disconnect::ByApplication, "", "en")
        .await;
    Ok(format!("sftp:{}", target.path(&file)))
}
