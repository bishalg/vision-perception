-- owner: WP-G.12
-- Scoring outcome of schema 1.8 sessions (SCH-5 of
-- docs/spec/modules/asd-gaze-robustness/technical.md; owner decision 2026-11-01).
--
-- `scoring_status` / `unscorable_reason` are derived at ingest from the 1.8 request and are
-- NULL for a 1.6 or 1.7 session (those always carry score numbers): nothing is defaulted
-- (CON-14). An `unscorable` session has no social preference score, so the column becomes
-- nullable; it stays required for every other session.

ALTER TABLE sessions
    ADD COLUMN scoring_status text
        CHECK (scoring_status IN ('scored', 'unscorable')),
    ADD COLUMN unscorable_reason text
        CHECK (unscorable_reason IN ('aggregation_failed', 'insufficient_valid_gaze',
                                     'tracking_unusable', 'calibration_failed',
                                     'no_scorable_clips', 'other')),
    ADD CONSTRAINT sessions_unscorable_reason_status_check
        CHECK (unscorable_reason IS NULL OR scoring_status IS NOT DISTINCT FROM 'unscorable');

ALTER TABLE sessions ALTER COLUMN social_preference DROP NOT NULL;
ALTER TABLE sessions ADD CONSTRAINT sessions_social_preference_scored_check
    CHECK (CASE WHEN scoring_status = 'unscorable' THEN social_preference IS NULL
                ELSE social_preference IS NOT NULL END);

-- Review auto-rule `unscorable` (every 1.8 session with scoringStatus `unscorable`).
ALTER TABLE review_items DROP CONSTRAINT review_items_reason_check;
ALTER TABLE review_items ADD CONSTRAINT review_items_reason_check
    CHECK (reason IN ('low_quality', 'flag_red', 'random_audit', 'norm_candidate', 'manual',
                      'low_tracking_quality', 'unscorable'));
