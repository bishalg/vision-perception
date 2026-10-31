-- owner: security audit fixes (2026-10-31)
-- Numbered 0015 on purpose: 0014 is taken by a package running in parallel.
--
-- Finding 4: org-scoped, newest-first list indexes.
--   * `GET /v1/orgs/{id}/audit` pages `audit_log` by `id DESC` within an org (the 0001
--     index is on `(org_id, ts)`).
--   * The worklist (`GET /v1/orgs/{id}/worklist`) pages `sessions` by
--     `(received_at, id) DESC` within an org (the 0001 index, used by the session list,
--     is on `started_at`).
CREATE INDEX IF NOT EXISTS audit_log_org_id_desc_idx ON audit_log (org_id, id DESC);
CREATE INDEX IF NOT EXISTS sessions_org_received_idx
    ON sessions (org_id, received_at DESC, id DESC);

-- Finding 7 (SIGN-5): the RFC 6238 time step of the last TOTP code accepted for a step-up.
-- A code of the same or an earlier step is refused, so an accepted code is spent. NULL: no
-- code accepted yet. Updated with a compare-and-set (`WHERE last_totp_step IS NULL OR
-- last_totp_step < $step`), so two concurrent requests with one code cannot both pass.
ALTER TABLE users ADD COLUMN last_totp_step bigint CHECK (last_totp_step >= 0);
