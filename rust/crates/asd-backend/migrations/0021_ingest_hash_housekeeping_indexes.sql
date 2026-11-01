-- owner: WP-B.12 / WP-B.3 (audit follow-up 2026-11-01). Additive only.
--
-- 1. `sessions.request_hash`: SHA-256 over the RFC 8785 form of the ingest body, the same
--    value `idempotency_keys.request_hash` holds. The housekeeping sweep deletes
--    idempotency keys after 7 days (HK-4); a later post of the same `sessionId` is then
--    answered from the session row itself: same hash -> `200` replay, different hash ->
--    `409 idempotency-key-reused`. NULL for sessions stored before this migration (ingest
--    then compares `summary` with the posted `rawPayload`).
--
-- 2. Indexes for the housekeeping sweep's range scans and anti-joins (HK-1..5), and the
--    expired pending TOTP enrolments it clears (AUTH-6).

ALTER TABLE sessions ADD COLUMN IF NOT EXISTS request_hash bytea
    CHECK (request_hash IS NULL OR octet_length(request_hash) = 32);

CREATE INDEX IF NOT EXISTS refresh_tokens_expires_idx ON refresh_tokens (expires_at);
CREATE INDEX IF NOT EXISTS jobs_finished_idx ON jobs (finished_at)
    WHERE status IN ('succeeded', 'failed', 'cancelled');
CREATE INDEX IF NOT EXISTS devices_enrol_code_idx ON devices (enrol_code_id)
    WHERE enrol_code_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS exports_job_idx ON exports (job_id) WHERE job_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS export_deliveries_job_idx ON export_deliveries (job_id)
    WHERE job_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS erasure_requests_job_idx ON erasure_requests (job_id)
    WHERE job_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS users_totp_pending_expires_idx ON users (totp_pending_expires_at)
    WHERE totp_pending_expires_at IS NOT NULL;
