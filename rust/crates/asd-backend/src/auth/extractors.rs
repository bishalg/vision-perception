//! owner: WP-B.2
//!
//! Axum extractors for both auth planes and the request context:
//!
//! - [`UserPrincipal`]: `Authorization: Bearer <JWT>` (user plane). The token proves who
//!   the caller is; the role, org and status are re-read from `users` on every request, so
//!   a disabled or demoted user loses access at once rather than after 15 minutes.
//! - [`Require<P>`]: a `UserPrincipal` authorised for [`Permission`] `P` by the matrix in
//!   [`super::rbac`]. On `/v1/orgs/{id}/...` routes the org in the path is checked first
//!   (another org: 404), then the role (403). On other routes only the role is checked here
//!   and the handler calls [`UserPrincipal::ensure_org`] on the loaded resource.
//! - [`DevicePrincipal`]: `Authorization: Bearer dvc_...` (device plane, AUTH-3).
//! - [`RequestMeta`]: client IP and user agent for audit rows and rate limits.
//! - [`ApiJson`]: `Json` whose rejections are problem+json `validation-failed` (CON-14).
//!
//! Every authentication failure is the same `401 unauthorized` problem.

use std::marker::PhantomData;
use std::net::{IpAddr, SocketAddr};

use asd_contract::api_v1::Role;
use axum::extract::rejection::JsonRejection;
use axum::extract::{
    ConnectInfo, FromRequest, FromRequestParts, MatchedPath, RawPathParams, Request,
};
use http::request::Parts;
use http::HeaderMap;
use serde::de::DeserializeOwned;
use uuid::Uuid;

use super::device_token;
use super::rbac::{self, Permission};
use crate::app::AppState;
use crate::error::{AppError, AppResult};
use crate::problem::InvalidParam;

/// The token after `Bearer ` in `Authorization`, if any.
pub fn bearer(headers: &HeaderMap) -> Option<&str> {
    let value = headers.get(http::header::AUTHORIZATION)?.to_str().ok()?;
    let (scheme, token) = value.split_once(' ')?;
    let token = token.trim();
    (scheme.eq_ignore_ascii_case("bearer") && !token.is_empty()).then_some(token)
}

// ---- user plane ------------------------------------------------------------------------

/// The signed-in console user. `org_id` is `None` only for `superadmin`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UserPrincipal {
    pub user_id: Uuid,
    pub org_id: Option<Uuid>,
    pub role: Role,
}

impl UserPrincipal {
    pub fn is_superadmin(&self) -> bool {
        self.role == Role::Superadmin
    }

    /// The caller's own org, or any org for `superadmin`; otherwise 404 (AUTH-4).
    pub fn ensure_org(&self, org: Uuid) -> AppResult<()> {
        if self.is_superadmin() || self.org_id == Some(org) {
            Ok(())
        } else {
            Err(AppError::NotFound)
        }
    }
}

impl FromRequestParts<AppState> for UserPrincipal {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, AppError> {
        if let Some(cached) = parts.extensions.get::<UserPrincipal>() {
            return Ok(*cached);
        }
        let token = bearer(&parts.headers).ok_or(AppError::Unauthorized)?;
        if token.starts_with(device_token::PREFIX) {
            return Err(AppError::Unauthorized);
        }
        let keys = state.config.auth.jwt.as_ref().ok_or_else(|| {
            tracing::error!("ASD_JWT_SIGNING_KEY is not configured; user plane unavailable");
            AppError::Unavailable
        })?;
        let claims = keys.verify(token).ok_or(AppError::Unauthorized)?;
        let user_id = Uuid::parse_str(&claims.sub).map_err(|_| AppError::Unauthorized)?;
        let row = sqlx::query!(
            "SELECT org_id, role, status FROM users WHERE id = $1",
            user_id
        )
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::Unauthorized)?;
        if row.status != "active" {
            return Err(AppError::Unauthorized);
        }
        let role = rbac::role_from_str(&row.role).ok_or(AppError::Unauthorized)?;
        let principal = UserPrincipal {
            user_id,
            org_id: row.org_id,
            role,
        };
        parts.extensions.insert(principal);
        Ok(principal)
    }
}

/// A user authorised for `P` (see the module docs for the 404/403 order).
#[derive(Debug, Clone, Copy)]
pub struct Require<P> {
    pub principal: UserPrincipal,
    /// The org from a `/v1/orgs/{id}/...` path, already checked.
    pub org_id: Option<Uuid>,
    _permission: PhantomData<P>,
}

impl<P> Require<P> {
    /// The checked path org. Panics only if the route has no `/orgs/{id}` prefix, which
    /// is a routing bug caught by the first test of that route.
    pub fn org(&self) -> Uuid {
        self.org_id
            .expect("Require::org on a route without /orgs/{id}")
    }

    /// For resource routes: 404 unless the resource's org is the caller's.
    pub fn ensure_org(&self, org: Uuid) -> AppResult<()> {
        self.principal.ensure_org(org)
    }
}

impl<P: Permission> FromRequestParts<AppState> for Require<P> {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, AppError> {
        let principal = UserPrincipal::from_request_parts(parts, state).await?;
        let org_id = path_org(parts, state).await?;
        rbac::authorize(&principal, P::ACTION, org_id)?;
        Ok(Self {
            principal,
            org_id,
            _permission: PhantomData,
        })
    }
}

/// The org id of a `/v1/orgs/{param}/...` route (whatever the parameter is called);
/// `None` for other routes. A malformed id is a 404, like an unknown one.
async fn path_org(parts: &mut Parts, state: &AppState) -> AppResult<Option<Uuid>> {
    let Some(template) = parts
        .extensions
        .get::<MatchedPath>()
        .map(|m| m.as_str().to_owned())
    else {
        return Ok(None);
    };
    let rest = template
        .strip_prefix("/v1/orgs/{")
        .or_else(|| template.strip_prefix("/orgs/{"));
    let Some(name) = rest
        .and_then(|r| r.split_once('}'))
        .map(|(name, _)| name.to_owned())
    else {
        return Ok(None);
    };
    let params = RawPathParams::from_request_parts(parts, state)
        .await
        .map_err(|_| AppError::NotFound)?;
    let value = params
        .iter()
        .find(|(k, _)| *k == name)
        .map(|(_, v)| v.to_owned())
        .ok_or(AppError::NotFound)?;
    Uuid::parse_str(&value)
        .map(Some)
        .map_err(|_| AppError::NotFound)
}

// ---- device plane ----------------------------------------------------------------------

/// The calling device (AUTH-3): a live `dvc_` token bound to `(device_id, org_id)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DevicePrincipal {
    pub device_id: Uuid,
    pub org_id: Uuid,
}

impl FromRequestParts<AppState> for DevicePrincipal {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, AppError> {
        if let Some(cached) = parts.extensions.get::<DevicePrincipal>() {
            return Ok(*cached);
        }
        let token = bearer(&parts.headers).ok_or(AppError::Unauthorized)?;
        let principal = device_token::authenticate(&state.db, token)
            .await?
            .ok_or(AppError::Unauthorized)?;
        parts.extensions.insert(principal);
        Ok(principal)
    }
}

// ---- request context -------------------------------------------------------------------

/// Who is on the other end, for `audit_log.ip`/`user_agent` and rate-limit keys.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RequestMeta {
    pub ip: Option<IpAddr>,
    pub user_agent: Option<String>,
}

impl RequestMeta {
    pub fn from_parts(parts: &Parts, trust_proxy_headers: bool) -> Self {
        Self {
            ip: client_ip(parts, trust_proxy_headers),
            user_agent: parts
                .headers
                .get(http::header::USER_AGENT)
                .and_then(|v| v.to_str().ok())
                .map(|ua| ua.chars().take(256).collect()),
        }
    }
}

impl FromRequestParts<AppState> for RequestMeta {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        Ok(Self::from_parts(
            parts,
            state.config.auth.trust_proxy_headers,
        ))
    }
}

/// The peer address (`into_make_service_with_connect_info`), or, behind a trusted proxy,
/// `CF-Connecting-IP` then the last `X-Forwarded-For` hop (the one the proxy appended).
pub fn client_ip(parts: &Parts, trust_proxy_headers: bool) -> Option<IpAddr> {
    if trust_proxy_headers {
        let header_ip = |name: &str| {
            parts
                .headers
                .get(name)
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.rsplit(',').next())
                .and_then(|v| v.trim().parse::<IpAddr>().ok())
        };
        if let Some(ip) = header_ip("cf-connecting-ip").or_else(|| header_ip("x-forwarded-for")) {
            return Some(ip);
        }
    }
    parts
        .extensions
        .get::<ConnectInfo<SocketAddr>>()
        .map(|ConnectInfo(addr)| addr.ip())
}

// ---- JSON bodies -----------------------------------------------------------------------

/// `axum::Json` with problem+json rejections: a malformed or incomplete body is
/// `400 validation-failed` with one `invalid_params` entry naming the problem.
#[derive(Debug, Clone, Copy, Default)]
pub struct ApiJson<T>(pub T);

impl<T, S> FromRequest<S> for ApiJson<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request(req: Request, state: &S) -> Result<Self, AppError> {
        match axum::Json::<T>::from_request(req, state).await {
            Ok(axum::Json(value)) => Ok(Self(value)),
            Err(rejection) => Err(json_rejection(rejection)),
        }
    }
}

fn json_rejection(rejection: JsonRejection) -> AppError {
    let (name, reason) = match &rejection {
        JsonRejection::MissingJsonContentType(_) => {
            ("content-type", "expected application/json".to_owned())
        }
        JsonRejection::JsonSyntaxError(_) => ("body", "malformed JSON".to_owned()),
        JsonRejection::JsonDataError(e) => ("body", e.body_text()),
        _ => ("body", rejection.body_text()),
    };
    AppError::Validation(vec![InvalidParam {
        name: name.into(),
        reason,
    }])
}

/// A JSON body that may be empty (`{}` semantics), e.g. `POST .../enrol-code`.
pub fn json_or_default<T: DeserializeOwned + Default>(bytes: &[u8]) -> AppResult<T> {
    if bytes.iter().all(u8::is_ascii_whitespace) {
        return Ok(T::default());
    }
    serde_json::from_slice(bytes).map_err(|e| {
        AppError::Validation(vec![InvalidParam {
            name: "body".into(),
            reason: e.to_string(),
        }])
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parts(headers: &[(&str, &str)], peer: Option<&str>) -> Parts {
        let mut b = http::Request::builder().uri("/");
        for (k, v) in headers {
            b = b.header(*k, *v);
        }
        let (mut parts, ()) = b.body(()).unwrap().into_parts();
        if let Some(p) = peer {
            parts
                .extensions
                .insert(ConnectInfo(p.parse::<SocketAddr>().unwrap()));
        }
        parts
    }

    #[test]
    fn bearer_parsing() {
        let h = |v: &str| {
            let mut m = HeaderMap::new();
            m.insert(http::header::AUTHORIZATION, v.parse().unwrap());
            m
        };
        assert_eq!(bearer(&h("Bearer abc")), Some("abc"));
        assert_eq!(bearer(&h("bearer  abc ")), Some("abc"));
        assert_eq!(bearer(&h("Basic abc")), None);
        assert_eq!(bearer(&h("Bearer ")), None);
        assert_eq!(bearer(&HeaderMap::new()), None);
    }

    #[test]
    fn client_ip_ignores_proxy_headers_unless_trusted() {
        let p = parts(
            &[
                ("x-forwarded-for", "1.1.1.1, 10.0.0.2"),
                ("cf-connecting-ip", "2.2.2.2"),
            ],
            Some("10.0.0.9:5000"),
        );
        assert_eq!(client_ip(&p, false), Some("10.0.0.9".parse().unwrap()));
        assert_eq!(client_ip(&p, true), Some("2.2.2.2".parse().unwrap()));
        let p = parts(&[("x-forwarded-for", "1.1.1.1, 10.0.0.2")], None);
        assert_eq!(
            client_ip(&p, true),
            Some("10.0.0.2".parse().unwrap()),
            "last hop"
        );
        assert_eq!(client_ip(&p, false), None);
        let p = parts(&[("x-forwarded-for", "garbage")], Some("[::1]:1"));
        assert_eq!(client_ip(&p, true), Some("::1".parse().unwrap()));
    }

    #[test]
    fn request_meta_truncates_the_user_agent() {
        let ua = "x".repeat(1000);
        let m = RequestMeta::from_parts(&parts(&[("user-agent", &ua)], None), false);
        assert_eq!(m.user_agent.unwrap().len(), 256);
    }

    #[test]
    fn empty_bodies_default() {
        #[derive(Default, serde::Deserialize, PartialEq, Debug)]
        struct B {
            #[serde(default)]
            n: Option<u32>,
        }
        assert_eq!(json_or_default::<B>(b"").unwrap(), B::default());
        assert_eq!(json_or_default::<B>(b" \n").unwrap(), B::default());
        assert_eq!(
            json_or_default::<B>(br#"{"n":3}"#).unwrap(),
            B { n: Some(3) }
        );
        assert!(json_or_default::<B>(b"{").is_err());
    }
}
