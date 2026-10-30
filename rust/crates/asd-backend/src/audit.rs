//! owner: WP-B.2
//!
//! AUTH-5: every mutating handler appends exactly one `audit_log` row **in its own
//! transaction**, so the row exists if and only if the change committed. The table is
//! append-only (`migrations/0002_auth_grants.sql`).
//!
//! `before`/`after` are hand-built snapshots of the fields that changed: never a password,
//! hash, token, enrol code or session payload. Usage:
//!
//! ```ignore
//! AuditEvent::user(&principal, "org.update", "organization", org_id)
//!     .org(org_id)
//!     .before(json!({"name": old}))
//!     .after(json!({"name": new}))
//!     .meta(&meta)
//!     .record(&mut tx)
//!     .await?;
//! ```

use std::net::IpAddr;

use asd_contract::api_v1::ActorType;
use serde_json::Value;
use sqlx::PgConnection;
use uuid::Uuid;

use crate::auth::extractors::{DevicePrincipal, RequestMeta, UserPrincipal};
use crate::error::AppResult;

pub fn actor_type_str(t: ActorType) -> &'static str {
    match t {
        ActorType::User => "user",
        ActorType::Device => "device",
        ActorType::System => "system",
    }
}

/// One audit row, built then [`record`](AuditEvent::record)ed.
#[derive(Debug, Clone, PartialEq)]
#[must_use = "an audit event does nothing until `.record(&mut tx)`"]
pub struct AuditEvent {
    pub actor_type: ActorType,
    pub actor_id: Option<String>,
    pub org_id: Option<Uuid>,
    pub action: String,
    pub target_type: String,
    pub target_id: Option<String>,
    pub before: Option<Value>,
    pub after: Option<Value>,
    pub ip: Option<IpAddr>,
    pub user_agent: Option<String>,
}

impl AuditEvent {
    pub fn new(
        actor_type: ActorType,
        actor_id: Option<String>,
        action: &str,
        target_type: &str,
        target_id: impl ToString,
    ) -> Self {
        Self {
            actor_type,
            actor_id,
            org_id: None,
            action: action.to_owned(),
            target_type: target_type.to_owned(),
            target_id: Some(target_id.to_string()),
            before: None,
            after: None,
            ip: None,
            user_agent: None,
        }
    }

    /// A console user's action; `org_id` defaults to the user's org (set it explicitly
    /// with [`org`](Self::org) when a superadmin acts on an org).
    pub fn user(
        principal: &UserPrincipal,
        action: &str,
        target_type: &str,
        target_id: impl ToString,
    ) -> Self {
        let mut e = Self::new(
            ActorType::User,
            Some(principal.user_id.to_string()),
            action,
            target_type,
            target_id,
        );
        e.org_id = principal.org_id;
        e
    }

    /// A user known only by id (login, refresh, logout).
    pub fn user_id(
        user_id: Uuid,
        org_id: Option<Uuid>,
        action: &str,
        target_type: &str,
        target_id: impl ToString,
    ) -> Self {
        let mut e = Self::new(
            ActorType::User,
            Some(user_id.to_string()),
            action,
            target_type,
            target_id,
        );
        e.org_id = org_id;
        e
    }

    pub fn device(
        device: &DevicePrincipal,
        action: &str,
        target_type: &str,
        target_id: impl ToString,
    ) -> Self {
        let mut e = Self::new(
            ActorType::Device,
            Some(device.device_id.to_string()),
            action,
            target_type,
            target_id,
        );
        e.org_id = Some(device.org_id);
        e
    }

    pub fn system(action: &str, target_type: &str, target_id: impl ToString) -> Self {
        Self::new(ActorType::System, None, action, target_type, target_id)
    }

    pub fn org(mut self, org_id: Uuid) -> Self {
        self.org_id = Some(org_id);
        self
    }

    pub fn before(mut self, snapshot: Value) -> Self {
        self.before = Some(snapshot);
        self
    }

    pub fn after(mut self, snapshot: Value) -> Self {
        self.after = Some(snapshot);
        self
    }

    pub fn meta(mut self, meta: &RequestMeta) -> Self {
        self.ip = meta.ip;
        self.user_agent = meta.user_agent.clone();
        self
    }

    /// Appends the row on `conn` (the handler's transaction); returns its id.
    pub async fn record(self, conn: &mut PgConnection) -> AppResult<i64> {
        let ip = self.ip.map(|ip| ip.to_string());
        let id = sqlx::query_scalar!(
            "INSERT INTO audit_log
               (actor_type, actor_id, org_id, action, target_type, target_id,
                before, after, ip, user_agent)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, CAST($9::text AS inet), $10)
             RETURNING id",
            actor_type_str(self.actor_type),
            self.actor_id,
            self.org_id,
            self.action,
            self.target_type,
            self.target_id,
            self.before,
            self.after,
            ip,
            self.user_agent,
        )
        .fetch_one(&mut *conn)
        .await?;
        Ok(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use asd_contract::api_v1::Role;

    #[test]
    fn builders_fill_actor_and_org() {
        let org = Uuid::new_v4();
        let p = UserPrincipal {
            user_id: Uuid::new_v4(),
            org_id: Some(org),
            role: Role::Admin,
        };
        let e = AuditEvent::user(&p, "org.update", "organization", org)
            .after(serde_json::json!({"name": "x"}))
            .meta(&RequestMeta {
                ip: Some("192.0.2.1".parse().unwrap()),
                user_agent: Some("ua".into()),
            });
        assert_eq!(e.actor_type, ActorType::User);
        assert_eq!(e.actor_id, Some(p.user_id.to_string()));
        assert_eq!(e.org_id, Some(org));
        assert_eq!(e.target_id, Some(org.to_string()));
        assert_eq!(e.ip, Some("192.0.2.1".parse().unwrap()));

        let root = UserPrincipal {
            user_id: Uuid::new_v4(),
            org_id: None,
            role: Role::Superadmin,
        };
        assert_eq!(
            AuditEvent::user(&root, "a", "b", "c").org(org).org_id,
            Some(org)
        );
        let d = DevicePrincipal {
            device_id: Uuid::new_v4(),
            org_id: org,
        };
        let e = AuditEvent::device(&d, "device.register", "device", d.device_id);
        assert_eq!(e.actor_type, ActorType::Device);
        assert_eq!(e.org_id, Some(org));
        assert_eq!(AuditEvent::system("x", "y", "z").actor_id, None);
    }
}
