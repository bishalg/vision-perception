-- owner: WP-B.6
-- Care events and erasure requests (device plane).
--   * one open erasure request per participant: a repeated request returns the open one
--     instead of stacking rows, and two concurrent requests cannot both insert.
--   * care event payloads are JSON objects (the summary reads keys from them).
--   * summary reads are per org and participant, newest first.
CREATE UNIQUE INDEX erasure_requests_one_open_idx ON erasure_requests (participant_id)
    WHERE completed_at IS NULL;
ALTER TABLE care_events ADD CONSTRAINT care_events_payload_object
    CHECK (jsonb_typeof(payload) = 'object');
CREATE INDEX care_events_org_participant_idx ON care_events (org_id, participant_id, occurred_at DESC);
