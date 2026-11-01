-- owner: security audit follow-ups (2026-10-31): TOTP seed encrypted at rest.
--
-- `users.totp_secret` (0001) held the RFC 6238 seed in clear. From 0016 on the service
-- reads and writes only:
--   * `totp_secret_ct`     AES-256-GCM, layout version || nonce || ciphertext || tag, under
--                          a key derived from ASD_DATA_KEK (`Keyring::seal_user_secret`),
--                          AAD "users:totp_secret_ct:00000000-0000-0000-0000-000000000000:{id}"
--                          (users are not org data: a superadmin has no org);
--   * `totp_secret_kek_id` fingerprint of the KEK it was sealed under, so a wrong KEK is
--                          diagnosed (503) instead of failing as a decryption error.
--
-- The KEK never reaches the database, so this SQL cannot encrypt. `asd-backend migrate`
-- converts the existing rows right after applying this migration
-- (`auth::totp::encrypt_legacy_secrets`): it seals each plaintext seed and sets
-- `totp_secret = NULL`, in the same run. Without ASD_DATA_KEK it refuses to finish while a
-- plaintext seed remains. Until a row is converted the service treats the user as enrolled
-- and refuses the code (fail closed). A later migration drops `totp_secret`.

-- Blank seeds never enrolled anyone (the old check was `btrim(totp_secret) <> ''`).
UPDATE users SET totp_secret = NULL WHERE btrim(totp_secret) = '';

ALTER TABLE users
    ADD COLUMN totp_secret_ct bytea,
    ADD COLUMN totp_secret_kek_id text,
    ADD CONSTRAINT users_totp_secret_ct_kek CHECK ((totp_secret_ct IS NULL) = (totp_secret_kek_id IS NULL)),
    -- A converted row keeps no plaintext copy.
    ADD CONSTRAINT users_totp_secret_one_form CHECK (totp_secret IS NULL OR totp_secret_ct IS NULL);
