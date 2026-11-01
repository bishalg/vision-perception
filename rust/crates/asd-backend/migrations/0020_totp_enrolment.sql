-- owner: WP-B.11 (ADR-0009, AUTH-6..10): self-service TOTP enrolment.
--
-- `POST /v1/users/me/totp/enrol` stores a *pending* seed until the user proves, with
-- `POST /v1/users/me/totp/confirm`, that the authenticator app holds it. Only then is it
-- moved to `totp_secret_ct` (0016). Login and step-up never read the pending columns.
--   * `totp_pending_ct`         AES-256-GCM like `totp_secret_ct`, sealed with
--                               `Keyring::seal_user_secret` under AAD
--                               "users:totp_pending_ct:00000000-0000-0000-0000-000000000000:{id}";
--   * `totp_pending_kek_id`     fingerprint of the KEK it was sealed under;
--   * `totp_pending_expires_at` enrol time + 10 minutes; past it, confirm answers 410.
-- All three are NULL for users without a pending enrolment.

ALTER TABLE users
    ADD COLUMN totp_pending_ct bytea,
    ADD COLUMN totp_pending_kek_id text,
    ADD COLUMN totp_pending_expires_at timestamptz,
    ADD CONSTRAINT users_totp_pending_ct_kek
        CHECK ((totp_pending_ct IS NULL) = (totp_pending_kek_id IS NULL)),
    ADD CONSTRAINT users_totp_pending_expiry
        CHECK ((totp_pending_ct IS NULL) = (totp_pending_expires_at IS NULL));
