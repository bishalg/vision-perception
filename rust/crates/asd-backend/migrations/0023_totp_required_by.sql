-- owner: WP-B.16 (ADR-0010 decision 5, AUTH-11, AUTH-12): mandatory TOTP per role.
--
-- `users.totp_required_by` is the deadline by which a user in a TOTP-required role
-- (`asd-contract::auth_policy::TOTP_REQUIRED_ROLES`) must have enrolled an authenticator.
-- It is set lazily by the service, never here: to `now() + TOTP_GRACE_DAYS` the first time
-- such a user without TOTP is seen (at login, in the same transaction as `last_login_at`;
-- when an org user is created in a required role; when a role changes into a required
-- one), and only while it is NULL. It is never moved, never cleared on enrolment or
-- `admin reset-totp`, and left in place when the role stops being required (the policy
-- keys on the current role, so the value is then inert).
--
-- No back-fill and no constraint: existing users start their grace period at their first
-- login after the release, which is also the first time they can see the nudge.

ALTER TABLE users ADD COLUMN totp_required_by timestamptz;
