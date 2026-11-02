//! owner: WP-B.16
//!
//! Mandatory two-step sign-in per role (AUTH-11..AUTH-14, ADR-0010 d5-d6).
//!
//! The policy is evaluated from database state on every user-plane request, not from a
//! token claim: [`super::extractors::UserPrincipal`] reads whether the caller has TOTP and
//! its `users.totp_required_by` deadline with the row it already loads, calls
//! [`evaluate`], and answers `403 totp-enrolment-required` ([`enrolment_required`]) when
//! the state is `Required` and the matched route is not one of the four
//! [`is_exempt_route`] templates. The deadline is set lazily ([`deadline_from`]) by login,
//! user creation and a role change into a required role; it is never moved or cleared.

use asd_contract::api_v1::Role;
use asd_contract::auth_policy::{is_required_role, TotpEnforcement, TOTP_GRACE_DAYS};
use asd_contract::problem::TOTP_ENROLMENT_REQUIRED;
use chrono::{DateTime, Duration, Utc};
use http::Method;

use crate::error::AppError;

/// The routes a `Required` user may still call (AUTH-13), as `(method, MatchedPath
/// template)`: read the profile, render the org shell, enrol and confirm.
pub const EXEMPT_ROUTES: &[(Method, &str)] = &[
    (Method::GET, "/v1/users/me"),
    (Method::POST, "/v1/users/me/totp/enrol"),
    (Method::POST, "/v1/users/me/totp/confirm"),
    (Method::GET, "/v1/orgs/{id}"),
];

/// AUTH-11: `NotRequired` when the role is not listed or TOTP is enrolled; `Grace` while
/// the deadline is unset or still ahead of `now`; `Required` once it has passed.
pub fn evaluate(
    role: Role,
    totp_enrolled: bool,
    totp_required_by: Option<DateTime<Utc>>,
    now: DateTime<Utc>,
) -> TotpEnforcement {
    if !is_required_role(role) || totp_enrolled {
        return TotpEnforcement::NotRequired;
    }
    match totp_required_by {
        Some(deadline) if deadline <= now => TotpEnforcement::Required,
        _ => TotpEnforcement::Grace,
    }
}

/// True when a `Required` user may call this route. Matched on the route template, never
/// the concrete path; a request with no template (the 404 fallback) is not exempt.
pub fn is_exempt_route(method: &Method, template: Option<&str>) -> bool {
    let Some(template) = template else {
        return false;
    };
    EXEMPT_ROUTES
        .iter()
        .any(|(m, t)| m == method && *t == template)
}

/// AUTH-12: the enrolment deadline for a user first seen at `now`.
pub fn deadline_from(now: DateTime<Utc>) -> DateTime<Utc> {
    now + Duration::days(TOTP_GRACE_DAYS)
}

/// AUTH-12: whether this moment should set the deadline: a required role, no TOTP, and
/// no deadline yet (it is never moved).
pub fn needs_deadline(
    role: Role,
    totp_enrolled: bool,
    totp_required_by: Option<DateTime<Utc>>,
) -> bool {
    is_required_role(role) && !totp_enrolled && totp_required_by.is_none()
}

/// The `403 totp-enrolment-required` problem (AUTH-13); `detail` names the deadline date.
pub fn enrolment_required(totp_required_by: Option<DateTime<Utc>>) -> AppError {
    let detail = match totp_required_by {
        Some(d) => format!(
            "your role requires two-step sign-in since {}; set up an authenticator app to continue",
            d.format("%Y-%m-%d")
        ),
        None => "your role requires two-step sign-in; set up an authenticator app to continue"
            .to_owned(),
    };
    AppError::ForbiddenWith {
        slug: TOTP_ENROLMENT_REQUIRED,
        detail,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 11, 2, 12, 0, 0).unwrap()
    }

    #[test]
    fn evaluate_truth_table() {
        use TotpEnforcement::{Grace, NotRequired, Required};
        let now = now();
        let past = Some(now - Duration::seconds(1));
        let exactly = Some(now);
        let future = Some(now + Duration::days(3));
        let listed = [Role::Owner, Role::Admin, Role::Clinician, Role::Superadmin];
        let unlisted = [Role::Researcher, Role::Reviewer, Role::Viewer];
        for role in listed {
            for deadline in [None, past, exactly, future] {
                assert_eq!(
                    evaluate(role, true, deadline, now),
                    NotRequired,
                    "{role:?} enrolled {deadline:?}"
                );
            }
            assert_eq!(evaluate(role, false, None, now), Grace, "{role:?}");
            assert_eq!(evaluate(role, false, future, now), Grace, "{role:?}");
            assert_eq!(evaluate(role, false, past, now), Required, "{role:?}");
            assert_eq!(
                evaluate(role, false, exactly, now),
                Required,
                "{role:?}: the deadline itself is past"
            );
        }
        for role in unlisted {
            for enrolled in [false, true] {
                for deadline in [None, past, exactly, future] {
                    assert_eq!(
                        evaluate(role, enrolled, deadline, now),
                        NotRequired,
                        "{role:?} {enrolled} {deadline:?}"
                    );
                }
            }
        }
        // The same row moves from grace to required as time passes.
        let d = now + Duration::days(1);
        assert_eq!(evaluate(Role::Admin, false, Some(d), now), Grace);
        assert_eq!(
            evaluate(Role::Admin, false, Some(d), now + Duration::days(2)),
            Required
        );
    }

    #[test]
    fn exempt_routes_are_the_four_templates() {
        assert!(is_exempt_route(&Method::GET, Some("/v1/users/me")));
        assert!(is_exempt_route(
            &Method::POST,
            Some("/v1/users/me/totp/enrol")
        ));
        assert!(is_exempt_route(
            &Method::POST,
            Some("/v1/users/me/totp/confirm")
        ));
        assert!(is_exempt_route(&Method::GET, Some("/v1/orgs/{id}")));
    }

    #[test]
    fn other_routes_are_not_exempt() {
        for (method, template) in [
            (Method::PATCH, Some("/v1/users/me")),
            (Method::DELETE, Some("/v1/users/me/totp")),
            (Method::GET, Some("/v1/orgs/{id}/sessions")),
            (Method::PATCH, Some("/v1/orgs/{id}")),
            (Method::GET, Some("/v1/orgs")),
            (Method::GET, Some("/v1/users/me/totp/enrol")),
            (Method::HEAD, Some("/v1/users/me")),
            // A concrete path is not a template.
            (
                Method::GET,
                Some("/v1/orgs/7c1d1f0e-0000-0000-0000-000000000000"),
            ),
            (Method::GET, None),
            (Method::POST, None),
        ] {
            assert!(!is_exempt_route(&method, template), "{method} {template:?}");
        }
    }

    #[test]
    fn the_deadline_is_fourteen_days_ahead() {
        assert_eq!(deadline_from(now()) - now(), Duration::days(14));
    }

    #[test]
    fn the_deadline_is_set_once_for_required_roles_without_totp() {
        let d = Some(now());
        assert!(needs_deadline(Role::Owner, false, None));
        assert!(needs_deadline(Role::Superadmin, false, None));
        assert!(!needs_deadline(Role::Owner, false, d), "never moved");
        assert!(!needs_deadline(Role::Owner, true, None), "enrolled");
        assert!(!needs_deadline(Role::Viewer, false, None), "not listed");
    }

    #[test]
    fn the_problem_names_the_deadline() {
        let p = enrolment_required(Some(now())).to_problem();
        assert_eq!(p.status, 403);
        assert!(
            p.type_uri.ends_with("/totp-enrolment-required"),
            "{}",
            p.type_uri
        );
        let body = serde_json::to_value(&p).unwrap();
        assert!(
            body["detail"].as_str().unwrap().contains("2026-11-02"),
            "{body}"
        );
    }
}
