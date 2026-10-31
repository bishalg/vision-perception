//! owner: WP-I.7
//!
//! Signed webhooks (DLV-2, DLV-3). The body is an event and a short-lived download link,
//! never clinical data or an MRN: receivers pull the artifact through the link (15
//! minutes). Signed per the Standard Webhooks scheme:
//!
//! ```text
//! webhook-id:        <delivery id>              (stable across retries: receivers dedupe)
//! webhook-timestamp: <unix seconds>
//! webhook-signature: v1,<base64(HMAC-SHA256(key, "{id}.{timestamp}.{body}"))>
//! ```
//!
//! where `key` is the base64-decoded part of the `whsec_...` secret.
//!
//! Transport: hyper's client over hyper-rustls with the platform trust store
//! (`rustls-native-certs`, G12). The connector's resolver is pinned to the addresses
//! [`netguard::resolve`] already checked, so the TCP connection goes to a checked address
//! while TLS still verifies the certificate for the URL's host name. Redirects are not
//! followed (the client never does); connect 10 s, whole exchange 30 s; at most 64 KiB of
//! the response body is read and then discarded.

use std::net::SocketAddr;
use std::sync::{Arc, OnceLock};
use std::task::{Context, Poll};
use std::time::Duration;

use aws_lc_rs::hmac;
use base64::Engine as _;
use bytes::Bytes;
use chrono::Utc;
use http::header::{CONTENT_TYPE, USER_AGENT};
use http::Request;
use http_body_util::{BodyExt, Full};
use hyper_util::client::legacy::connect::dns::Name;
use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::client::legacy::Client;
use hyper_util::rt::TokioExecutor;
use serde::Serialize;

use super::netguard::{self, Channel};
use super::{DeliveryFailure, ErrorClass};

pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
pub const TOTAL_TIMEOUT: Duration = Duration::from_secs(30);
/// Response bytes read (then discarded) at most.
pub const MAX_RESPONSE_BYTES: usize = 64 * 1024;
const USER_AGENT_VALUE: &str = "curv-asd-webhook/1";

/// The token of the synthetic record a connection test announces (INTG-4).
pub const TEST_TOKEN: &str = "TEST-0000";

/// The link a receiver downloads the artifact from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DownloadLink {
    pub url: String,
    pub expires_at: String,
}

/// INTG-4: what a test event names instead of a real export.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TestRecord {
    pub participant_token: &'static str,
}

/// The DLV-2 event body. Every field is an identifier or a format name: no clinical data,
/// no MRN, no participant token (except the synthetic `TEST-0000` of a test).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WebhookEvent {
    #[serde(rename = "type")]
    pub event_type: &'static str,
    /// The delivery id (also `webhook-id`).
    pub id: String,
    pub org_id: String,
    pub export_id: String,
    pub format: String,
    pub mode: String,
    pub created_at: String,
    pub download: Option<DownloadLink>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub test: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub test_record: Option<TestRecord>,
}

impl WebhookEvent {
    pub fn to_body(&self) -> Vec<u8> {
        serde_json::to_vec(self).expect("plain data")
    }
}

/// `v1,<base64 HMAC-SHA256(key, "{msg_id}.{timestamp}.{body}")>`.
pub fn signature(key: &[u8], msg_id: &str, timestamp: i64, body: &[u8]) -> String {
    let key = hmac::Key::new(hmac::HMAC_SHA256, key);
    let mut ctx = hmac::Context::with_key(&key);
    ctx.update(msg_id.as_bytes());
    ctx.update(b".");
    ctx.update(timestamp.to_string().as_bytes());
    ctx.update(b".");
    ctx.update(body);
    format!(
        "v1,{}",
        base64::engine::general_purpose::STANDARD.encode(ctx.sign().as_ref())
    )
}

/// Resolver of the HTTP connector: always the addresses netguard checked (the connector
/// sets the URL's port on them).
#[derive(Clone)]
pub struct PinnedResolver(Arc<Vec<SocketAddr>>);

impl tower::Service<Name> for PinnedResolver {
    type Response = std::vec::IntoIter<SocketAddr>;
    type Error = std::io::Error;
    type Future = std::future::Ready<Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, _: Name) -> Self::Future {
        std::future::ready(Ok(self.0.as_ref().clone().into_iter()))
    }
}

/// The rustls client config with the platform's roots, built once per process.
fn tls_config() -> Option<Arc<rustls::ClientConfig>> {
    static CONFIG: OnceLock<Option<Arc<rustls::ClientConfig>>> = OnceLock::new();
    CONFIG
        .get_or_init(|| {
            let mut roots = rustls::RootCertStore::empty();
            let loaded = rustls_native_certs::load_native_certs();
            for cert in loaded.certs {
                let _ = roots.add(cert);
            }
            if roots.is_empty() {
                tracing::error!("no platform CA certificates found; outbound https will fail");
            }
            let provider = Arc::new(rustls::crypto::aws_lc_rs::default_provider());
            rustls::ClientConfig::builder_with_provider(provider)
                .with_safe_default_protocol_versions()
                .ok()
                .map(|b| Arc::new(b.with_root_certificates(roots).with_no_client_auth()))
        })
        .clone()
}

fn classify(e: &(dyn std::error::Error + 'static)) -> ErrorClass {
    let mut cur: Option<&(dyn std::error::Error + 'static)> = Some(e);
    while let Some(err) = cur {
        if err.downcast_ref::<rustls::Error>().is_some() {
            return ErrorClass::TlsFailed;
        }
        if let Some(io) = err.downcast_ref::<std::io::Error>() {
            if io.kind() == std::io::ErrorKind::TimedOut {
                return ErrorClass::Timeout;
            }
            if io
                .get_ref()
                .is_some_and(|inner| inner.downcast_ref::<rustls::Error>().is_some())
            {
                return ErrorClass::TlsFailed;
            }
        }
        cur = err.source();
    }
    let text = e.to_string();
    if text.contains("timed out") {
        ErrorClass::Timeout
    } else {
        ErrorClass::ConnectFailed
    }
}

/// POSTs a signed `body` to `url`. `Ok` carries the remote reference (`http 204`); a
/// non-2xx answer is `http-status` (or `http-redirect`) with the status as reference.
pub async fn post(
    url: &str,
    key: &[u8],
    msg_id: &str,
    body: Vec<u8>,
) -> Result<String, DeliveryFailure> {
    let url = netguard::parse_webhook_url(url).map_err(|_| ErrorClass::InvalidConfig)?;
    let addrs = netguard::resolve(&url.host, url.port, Channel::Http).await?;
    let tls = tls_config().ok_or(ErrorClass::TlsFailed)?;

    let mut http = HttpConnector::new_with_resolver(PinnedResolver(Arc::new(addrs)));
    http.enforce_http(false);
    http.set_connect_timeout(Some(CONNECT_TIMEOUT));
    http.set_nodelay(true);
    let builder = hyper_rustls::HttpsConnectorBuilder::new().with_tls_config((*tls).clone());
    let https = if url.https {
        builder.https_only().enable_http1().wrap_connector(http)
    } else {
        // Plain http is reachable only after `netguard::allow_loopback_for_tests`.
        builder.https_or_http().enable_http1().wrap_connector(http)
    };
    let client: Client<_, Full<Bytes>> = Client::builder(TokioExecutor::new())
        .pool_max_idle_per_host(0)
        .build(https);

    let timestamp = Utc::now().timestamp();
    let request = Request::post(url.uri.clone())
        .header(CONTENT_TYPE, "application/json")
        .header(USER_AGENT, USER_AGENT_VALUE)
        .header("webhook-id", msg_id)
        .header("webhook-timestamp", timestamp.to_string())
        .header(
            "webhook-signature",
            signature(key, msg_id, timestamp, &body),
        )
        .body(Full::new(Bytes::from(body)))
        .map_err(|_| ErrorClass::InvalidConfig)?;

    let exchange = async {
        let response = client
            .request(request)
            .await
            .map_err(|e| DeliveryFailure::from(classify(&e)))?;
        let status = response.status();
        // Read a bounded amount, then drop the connection (DLV-3); the body is never kept.
        let mut body = response.into_body();
        let mut read = 0usize;
        while read < MAX_RESPONSE_BYTES {
            match body.frame().await {
                Some(Ok(frame)) => read += frame.data_ref().map_or(0, Bytes::len),
                _ => break,
            }
        }
        Ok::<_, DeliveryFailure>(status)
    };
    let status = tokio::time::timeout(TOTAL_TIMEOUT, exchange)
        .await
        .map_err(|_| ErrorClass::Timeout)??;
    let remote_ref = format!("http {}", status.as_u16());
    if status.is_success() {
        Ok(remote_ref)
    } else {
        Err(DeliveryFailure {
            class: if status.is_redirection() {
                ErrorClass::HttpRedirect
            } else {
                ErrorClass::HttpStatus
            },
            remote_ref: Some(remote_ref),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_body_shape() {
        let e = WebhookEvent {
            event_type: "export.ready",
            id: "d".into(),
            org_id: "o".into(),
            export_id: "e".into(),
            format: "csv".into(),
            mode: "identified".into(),
            created_at: "2026-10-31T00:00:00.000Z".into(),
            download: None,
            test: None,
            test_record: None,
        };
        let v: serde_json::Value = serde_json::from_slice(&e.to_body()).unwrap();
        assert_eq!(v["type"], "export.ready");
        assert!(v.get("test").is_none());
        assert!(v["download"].is_null());
    }
}
