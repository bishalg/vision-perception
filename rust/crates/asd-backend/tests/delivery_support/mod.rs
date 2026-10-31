//! owner: WP-I.7
//!
//! Fixtures for the delivery tests: a local webhook receiver (plain HTTP on loopback, so
//! the binaries that use it call `netguard::allow_loopback_for_tests`), an in-process SFTP
//! server (`russh` + `russh-sftp`, in-memory files, pinned host key, one authorised key),
//! a participant fixture with a public base URL, and destination helpers.
#![allow(dead_code)]

use std::collections::{BTreeMap, HashMap};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU16, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use asd_backend::app::AppState;
use asd_backend::blob::MemoryBlobStore;
use asd_backend::config::Config;
use asd_backend::interop::ssh_keys;
use axum::body::Bytes;
use axum::extract::State;
use axum::response::{IntoResponse, Response};
use axum::Router;
use base64::Engine as _;
use http::{HeaderMap, Method, StatusCode, Uri};
use russh::keys::PrivateKey;
use russh::server::{Auth, Msg, Session, VerifiedPublicKey};
use russh::{Channel, ChannelId};
use russh_sftp::protocol::{
    Attrs, File, FileAttributes, Handle, Name, OpenFlags, Status, StatusCode as SftpStatus,
};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::common::{self, router, TestDb};
use crate::participant_support::Ctx;

/// The public origin the fixtures configure (`ASD_PUBLIC_BASE_URL`).
pub const PUBLIC_BASE: &str = "https://api.curv.test";

/// A webhook secret (`whsec_` + 32 bytes).
pub fn webhook_secret() -> String {
    format!(
        "whsec_{}",
        base64::engine::general_purpose::STANDARD.encode(Uuid::new_v4().as_bytes().repeat(2))
    )
}

/// `common::test_config` plus `ASD_PUBLIC_BASE_URL`.
pub fn config(database_url: &str) -> Config {
    Config::from_lookup(|key| match key {
        "DATABASE_URL" => Some(database_url.to_owned()),
        "ASD_BLOB_BACKEND" => Some("memory".to_owned()),
        "ASD_DB_ACQUIRE_TIMEOUT_MS" => Some("500".to_owned()),
        "ASD_JWT_SIGNING_KEY" => Some(common::test_jwt_key().to_owned()),
        "ASD_DATA_KEK" => Some(common::test_data_kek().to_owned()),
        "ASD_PUBLIC_BASE_URL" => Some(PUBLIC_BASE.to_owned()),
        _ => None,
    })
    .expect("test config is valid")
}

/// A participant fixture (`participant_support::Ctx`) whose config has a public base URL.
pub async fn ctx() -> Option<Ctx> {
    let db = TestDb::migrated().await?;
    let state = AppState::new(
        db.pool.clone(),
        Arc::new(MemoryBlobStore::new()),
        config(common::UNREACHABLE_DB_URL),
    );
    let app = router(state.clone());
    let org = common::auth::seed_org(&db.pool, "Clinic A").await;
    Some(Ctx {
        db,
        state,
        app,
        org,
    })
}

impl Ctx {
    /// `POST /v1/orgs/{org}/integrations`; expects 201 and returns the destination.
    pub async fn destination(&self, bearer: &str, body: Value) -> Value {
        let (s, b) = self
            .call(
                Method::POST,
                &format!("/v1/orgs/{}/integrations", self.org),
                Some(body),
                bearer,
            )
            .await;
        assert_eq!(s, StatusCode::CREATED, "{b}");
        b
    }

    /// A webhook destination at `url` with a secret; returns (id, secret).
    pub async fn webhook_destination(
        &self,
        bearer: &str,
        url: &str,
        extra: Value,
    ) -> (String, String) {
        let mut config = json!({"url": url});
        if let Value::Object(m) = extra {
            for (k, v) in m {
                config[k] = v;
            }
        }
        let d = self
            .destination(
                bearer,
                json!({"kind": "webhook", "name": "HIS hook", "environment": "production",
                       "config": config}),
            )
            .await;
        let id = d["id"].as_str().unwrap().to_owned();
        let secret = webhook_secret();
        let (s, b) = self
            .call(
                Method::PUT,
                &format!("/v1/integrations/{id}/secret"),
                Some(json!({"secret": secret})),
                bearer,
            )
            .await;
        assert_eq!(s, StatusCode::OK, "{b}");
        (id, secret)
    }

    /// The delivery rows of an export: (status, attempts, error class, remote_ref).
    pub async fn deliveries_of(
        &self,
        export_id: &str,
    ) -> Vec<(String, i32, Option<String>, Option<String>)> {
        sqlx::query_as(
            "SELECT status, attempts, last_error_class, remote_ref FROM export_deliveries
             WHERE export_id = $1::uuid ORDER BY created_at, id",
        )
        .bind(export_id)
        .fetch_all(&self.db.pool)
        .await
        .unwrap()
    }

    /// Makes every queued job due now (skips the backoff wait).
    pub async fn make_jobs_due(&self) {
        sqlx::query("UPDATE jobs SET run_after = now() WHERE status = 'queued'")
            .execute(&self.db.pool)
            .await
            .unwrap();
    }

    /// Seconds until the queued delivery job of `export_id` is due.
    pub async fn delivery_job_delay(&self, export_id: &str) -> f64 {
        sqlx::query_scalar(
            "SELECT extract(epoch FROM j.run_after - now())::float8 FROM jobs j
             JOIN export_deliveries d ON d.job_id = j.id
             WHERE d.export_id = $1::uuid AND j.status = 'queued'",
        )
        .bind(export_id)
        .fetch_one(&self.db.pool)
        .await
        .unwrap()
    }
}

// ---- webhook receiver --------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Hit {
    pub path: String,
    pub headers: HeaderMap,
    pub body: Vec<u8>,
}

impl Hit {
    pub fn header(&self, name: &str) -> &str {
        self.headers
            .get(name)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
    }

    pub fn json(&self) -> Value {
        serde_json::from_slice(&self.body).unwrap()
    }
}

#[derive(Clone)]
struct ReceiverState {
    hits: Arc<Mutex<Vec<Hit>>>,
    status: Arc<AtomicU16>,
    body: Arc<Mutex<String>>,
}

/// A local HTTP receiver recording every request. Answers `status` (default 200) with
/// `body`; a 3xx answer carries `Location: /redirected`.
pub struct Receiver {
    pub base: String,
    state: ReceiverState,
}

async fn record(
    State(s): State<ReceiverState>,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    s.hits.lock().unwrap().push(Hit {
        path: uri.path().to_owned(),
        headers,
        body: body.to_vec(),
    });
    let status = StatusCode::from_u16(s.status.load(Ordering::SeqCst)).unwrap();
    let text = s.body.lock().unwrap().clone();
    if status.is_redirection() {
        (status, [(http::header::LOCATION, "/redirected")], text).into_response()
    } else {
        (status, text).into_response()
    }
}

impl Receiver {
    pub async fn start() -> Self {
        let state = ReceiverState {
            hits: Arc::new(Mutex::new(Vec::new())),
            status: Arc::new(AtomicU16::new(200)),
            body: Arc::new(Mutex::new(String::new())),
        };
        let app = Router::new().fallback(record).with_state(state.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        Self {
            base: format!("http://{addr}"),
            state,
        }
    }

    pub fn url(&self, path: &str) -> String {
        format!("{}{path}", self.base)
    }

    pub fn respond(&self, status: u16, body: &str) {
        self.state.status.store(status, Ordering::SeqCst);
        *self.state.body.lock().unwrap() = body.to_owned();
    }

    pub fn hits(&self) -> Vec<Hit> {
        self.state.hits.lock().unwrap().clone()
    }
}

// ---- Standard Webhooks reference verification --------------------------------------------

/// Verifies `webhook-signature` the way a receiver would (Standard Webhooks reference):
/// HMAC-SHA256 over `"{id}.{timestamp}.{body}"` with the base64-decoded secret.
pub fn verify_standard_webhook(secret: &str, hit: &Hit) -> bool {
    let key = base64::engine::general_purpose::STANDARD
        .decode(secret.strip_prefix("whsec_").unwrap())
        .unwrap();
    let mut signed = Vec::new();
    signed.extend_from_slice(hit.header("webhook-id").as_bytes());
    signed.push(b'.');
    signed.extend_from_slice(hit.header("webhook-timestamp").as_bytes());
    signed.push(b'.');
    signed.extend_from_slice(&hit.body);
    let tag = aws_lc_rs::hmac::sign(
        &aws_lc_rs::hmac::Key::new(aws_lc_rs::hmac::HMAC_SHA256, &key),
        &signed,
    );
    let expected = format!(
        "v1,{}",
        base64::engine::general_purpose::STANDARD.encode(tag.as_ref())
    );
    hit.header("webhook-signature")
        .split(' ')
        .any(|s| s == expected)
}

// ---- in-process SFTP server --------------------------------------------------------------

/// What the fake SFTP server saw and holds.
#[derive(Default)]
pub struct SftpState {
    pub files: BTreeMap<String, Vec<u8>>,
    /// `open {path}`, `rename {from} -> {to}`, `remove {path}`, `auth {user}`.
    pub ops: Vec<String>,
}

pub struct SftpServer {
    pub addr: SocketAddr,
    /// `SHA256:...` of the server's host key.
    pub host_key_fingerprint: String,
    pub state: Arc<Mutex<SftpState>>,
    authorized: Arc<Mutex<Option<String>>>,
}

impl SftpServer {
    pub async fn start() -> Self {
        let host = ssh_keys::generate().unwrap();
        let host_key: PrivateKey = ssh_keys::private_key(host.seed.as_ref()).unwrap();
        let config = Arc::new(russh::server::Config {
            keys: vec![host_key],
            auth_rejection_time: Duration::from_millis(10),
            auth_rejection_time_initial: Some(Duration::ZERO),
            inactivity_timeout: Some(Duration::from_secs(30)),
            ..Default::default()
        });
        let state = Arc::new(Mutex::new(SftpState::default()));
        let authorized = Arc::new(Mutex::new(None));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let (st, auth) = (state.clone(), authorized.clone());
        tokio::spawn(async move {
            loop {
                let Ok((socket, _)) = listener.accept().await else {
                    break;
                };
                let handler = SshHandler {
                    state: st.clone(),
                    authorized: auth.clone(),
                    channels: HashMap::new(),
                };
                let cfg = config.clone();
                tokio::spawn(async move {
                    if let Ok(session) = russh::server::run_stream(cfg, socket, handler).await {
                        let _ = session.await;
                    }
                });
            }
        });
        Self {
            addr,
            host_key_fingerprint: host.fingerprint,
            state,
            authorized,
        }
    }

    /// The OpenSSH public key the server accepts (`ssh-ed25519 AAAA...`).
    pub fn authorize(&self, public_key: &str) {
        *self.authorized.lock().unwrap() = Some(public_key.to_owned());
    }

    pub fn files(&self) -> BTreeMap<String, Vec<u8>> {
        self.state.lock().unwrap().files.clone()
    }

    pub fn ops(&self) -> Vec<String> {
        self.state.lock().unwrap().ops.clone()
    }

    /// INTG-3 config for this server.
    pub fn config(&self, fingerprint: &str) -> Value {
        json!({
            "host": self.addr.ip().to_string(),
            "port": self.addr.port(),
            "username": "curv",
            "remote_dir": "/inbound",
            "host_key_sha256": fingerprint,
        })
    }
}

struct SshHandler {
    state: Arc<Mutex<SftpState>>,
    authorized: Arc<Mutex<Option<String>>>,
    channels: HashMap<ChannelId, Channel<Msg>>,
}

impl russh::server::Handler for SshHandler {
    type Error = russh::Error;

    async fn auth_publickey(
        &mut self,
        user: &str,
        key: VerifiedPublicKey<'_>,
    ) -> Result<Auth, Self::Error> {
        self.state.lock().unwrap().ops.push(format!("auth {user}"));
        let offered = ssh_keys::public_openssh(key.public_key);
        let ok = offered.is_some() && *self.authorized.lock().unwrap() == offered;
        Ok(if ok { Auth::Accept } else { Auth::reject() })
    }

    async fn channel_open_session(
        &mut self,
        channel: Channel<Msg>,
        reply: russh::server::ChannelOpenHandle,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        self.channels.insert(channel.id(), channel);
        reply.accept().await;
        Ok(())
    }

    async fn channel_eof(
        &mut self,
        channel: ChannelId,
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        session.close(channel)?;
        Ok(())
    }

    async fn subsystem_request(
        &mut self,
        channel_id: ChannelId,
        name: &str,
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        if name == "sftp" {
            if let Some(channel) = self.channels.remove(&channel_id) {
                session.channel_success(channel_id)?;
                let fs = MemFs {
                    state: self.state.clone(),
                    open: HashMap::new(),
                    next: 0,
                };
                russh_sftp::server::run(channel.into_stream(), fs).await;
                return Ok(());
            }
        }
        session.channel_failure(channel_id)?;
        Ok(())
    }
}

struct MemFs {
    state: Arc<Mutex<SftpState>>,
    /// handle -> path
    open: HashMap<String, String>,
    next: u64,
}

fn ok_status(id: u32) -> Status {
    Status {
        id,
        status_code: SftpStatus::Ok,
        error_message: "Ok".into(),
        language_tag: "en-US".into(),
    }
}

impl russh_sftp::server::Handler for MemFs {
    type Error = SftpStatus;

    fn unimplemented(&self) -> Self::Error {
        SftpStatus::OpUnsupported
    }

    async fn open(
        &mut self,
        id: u32,
        filename: String,
        pflags: OpenFlags,
        _attrs: FileAttributes,
    ) -> Result<Handle, Self::Error> {
        let mut st = self.state.lock().unwrap();
        st.ops.push(format!("open {filename}"));
        if pflags.contains(OpenFlags::TRUNCATE) || !st.files.contains_key(&filename) {
            st.files.insert(filename.clone(), Vec::new());
        }
        self.next += 1;
        let handle = format!("h{}", self.next);
        self.open.insert(handle.clone(), filename);
        Ok(Handle { id, handle })
    }

    async fn write(
        &mut self,
        id: u32,
        handle: String,
        offset: u64,
        data: Vec<u8>,
    ) -> Result<Status, Self::Error> {
        let path = self.open.get(&handle).ok_or(SftpStatus::Failure)?.clone();
        let mut st = self.state.lock().unwrap();
        let file = st.files.entry(path).or_default();
        let at = usize::try_from(offset).unwrap();
        if file.len() < at + data.len() {
            file.resize(at + data.len(), 0);
        }
        file[at..at + data.len()].copy_from_slice(&data);
        Ok(ok_status(id))
    }

    async fn close(&mut self, id: u32, handle: String) -> Result<Status, Self::Error> {
        self.open.remove(&handle);
        Ok(ok_status(id))
    }

    async fn remove(&mut self, id: u32, filename: String) -> Result<Status, Self::Error> {
        let mut st = self.state.lock().unwrap();
        st.ops.push(format!("remove {filename}"));
        if st.files.remove(&filename).is_some() {
            Ok(ok_status(id))
        } else {
            Err(SftpStatus::NoSuchFile)
        }
    }

    async fn rename(
        &mut self,
        id: u32,
        oldpath: String,
        newpath: String,
    ) -> Result<Status, Self::Error> {
        let mut st = self.state.lock().unwrap();
        st.ops.push(format!("rename {oldpath} -> {newpath}"));
        if st.files.contains_key(&newpath) {
            return Err(SftpStatus::Failure); // SFTP v3: the target must not exist
        }
        let data = st.files.remove(&oldpath).ok_or(SftpStatus::NoSuchFile)?;
        st.files.insert(newpath, data);
        Ok(ok_status(id))
    }

    async fn realpath(&mut self, id: u32, path: String) -> Result<Name, Self::Error> {
        Ok(Name {
            id,
            files: vec![File::dummy(path)],
        })
    }

    async fn stat(&mut self, id: u32, path: String) -> Result<Attrs, Self::Error> {
        let st = self.state.lock().unwrap();
        let len = st.files.get(&path).ok_or(SftpStatus::NoSuchFile)?.len();
        let attrs = FileAttributes {
            size: Some(len as u64),
            ..Default::default()
        };
        Ok(Attrs { id, attrs })
    }

    async fn fstat(&mut self, id: u32, handle: String) -> Result<Attrs, Self::Error> {
        let path = self.open.get(&handle).ok_or(SftpStatus::Failure)?.clone();
        self.stat(id, path).await
    }
}
