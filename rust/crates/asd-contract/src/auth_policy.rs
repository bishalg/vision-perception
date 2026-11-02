//! owner: WP-B.16
//!
//! Mandatory two-step sign-in per role (AUTH-11, ADR-0010 d5): which roles must enrol a
//! TOTP authenticator, how long the grace period is, and the enforcement state that
//! `GET /v1/users/me` reports. The roles and the grace period are owner decisions (O-10,
//! recommended defaults), so they are constants here and not environment settings; the
//! schema export publishes them as `$defs/TotpPolicy` in `api-v1.json`.
//!
//! The backend evaluates the state per request (`asd-backend::auth::totp_policy`); this
//! crate only holds the shared names so the console and the backend spell them once.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::api_v1::Role;

/// Roles that must enrol TOTP once their grace period has passed (O-10 default).
pub const TOTP_REQUIRED_ROLES: &[Role] =
    &[Role::Owner, Role::Admin, Role::Clinician, Role::Superadmin];

/// Days between the first time a required user without TOTP is seen (login, creation or a
/// role change into a required role) and the enrolment deadline (O-10 default).
pub const TOTP_GRACE_DAYS: i64 = 14;

/// True when `role` is one of [`TOTP_REQUIRED_ROLES`].
pub fn is_required_role(role: Role) -> bool {
    TOTP_REQUIRED_ROLES.contains(&role)
}

/// The caller's TOTP enforcement state (`UserProfile.totp_enforcement`, AUTH-11).
///
/// - `not_required`: the role is not in the required list, or TOTP is enrolled;
/// - `grace`: required, not enrolled, and the deadline is unset or still ahead (the
///   console shows a nudge with the date);
/// - `required`: the deadline has passed; every route except reading the profile, the
///   org shell and the enrolment routes answers `403 totp-enrolment-required`.
///
/// Older backends do not send the field; clients default it to `not_required`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TotpEnforcement {
    #[default]
    NotRequired,
    Grace,
    Required,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn required_roles_are_the_owner_defaults() {
        for role in [Role::Owner, Role::Admin, Role::Clinician, Role::Superadmin] {
            assert!(is_required_role(role), "{role:?}");
        }
        for role in [Role::Researcher, Role::Reviewer, Role::Viewer] {
            assert!(!is_required_role(role), "{role:?}");
        }
        assert_eq!(TOTP_GRACE_DAYS, 14);
    }

    #[test]
    fn enforcement_is_snake_case_and_defaults_to_not_required() {
        assert_eq!(
            serde_json::to_value(TotpEnforcement::NotRequired).unwrap(),
            serde_json::json!("not_required")
        );
        assert_eq!(
            serde_json::to_value(TotpEnforcement::Grace).unwrap(),
            serde_json::json!("grace")
        );
        assert_eq!(
            serde_json::to_value(TotpEnforcement::Required).unwrap(),
            serde_json::json!("required")
        );
        assert_eq!(TotpEnforcement::default(), TotpEnforcement::NotRequired);
    }

    #[test]
    fn a_profile_from_an_older_backend_still_parses() {
        let old = serde_json::json!({
            "id": "u1", "email": "a@b.org", "role": "owner", "org_id": "o1",
            "display_name": null, "professional_registration": null,
        });
        let p: crate::clinical::UserProfile = serde_json::from_value(old).unwrap();
        assert_eq!(p.totp_enforcement, TotpEnforcement::NotRequired);
        assert_eq!(p.totp_required_by, None);
    }

    #[test]
    fn the_api_schema_publishes_the_enum_the_policy_and_the_slug() {
        let api = crate::schema::api_v1();
        let defs = &api["$defs"];
        assert_eq!(
            defs["TotpEnforcement"]["enum"],
            serde_json::json!(["not_required", "grace", "required"]),
            "{}",
            defs["TotpEnforcement"]
        );
        let profile = &defs["UserProfile"]["properties"];
        assert!(profile["totp_enforcement"].is_object(), "{profile}");
        assert!(profile["totp_required_by"].is_object(), "{profile}");
        let required = defs["UserProfile"]["required"].as_array().unwrap();
        assert!(
            !required
                .iter()
                .any(|f| f == "totp_enforcement" || f == "totp_required_by"),
            "new fields are optional for older clients: {required:?}"
        );
        let policy = &defs["TotpPolicy"]["properties"];
        assert_eq!(
            policy["required_roles"]["const"],
            serde_json::json!(["owner", "admin", "clinician", "superadmin"])
        );
        assert_eq!(
            policy["grace_days"]["const"],
            serde_json::json!(TOTP_GRACE_DAYS)
        );
        assert!(defs["ProblemSlug"]["enum"]
            .as_array()
            .unwrap()
            .iter()
            .any(|s| s == crate::problem::TOTP_ENROLMENT_REQUIRED));
    }
}
