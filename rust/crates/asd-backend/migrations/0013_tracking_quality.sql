-- owner: WP-G.11
-- Tracking quality of schema 1.7 sessions (SCH-2, SCH-4 of
-- docs/spec/modules/asd-gaze-robustness/technical.md; ADR-0007). Numbered 0013 on purpose:
-- 0009/0010 are reserved for the interop/SMART packages and 0011/0012 for clinician
-- packages; sqlx applies a lower number added later as long as it was never applied.
--
-- Every column is derived at ingest from `rawPayload.trackingQuality` (like ING-2) and is
-- NULL for a 1.6 session or a 1.7 session without a tracking-quality record: nothing is
-- defaulted (CON-14). Enum values are spelled as in the record (`screenAffine`, `good`).

ALTER TABLE sessions
    ADD COLUMN tracking_verdict text
        CHECK (tracking_verdict IN ('good', 'fair', 'poor', 'unusable')),
    -- QS-2 `estimatedAccuracyDeg`, degrees of visual angle.
    ADD COLUMN tracking_accuracy_deg double precision
        CHECK (tracking_accuracy_deg >= 0),
    ADD COLUMN calibration_quality text
        CHECK (calibration_quality IN ('good', 'fair', 'poor', 'failed', 'skipped')),
    ADD COLUMN calibration_model text,
    ADD COLUMN tracking_pipeline_version text,
    -- `trackingQuality.device.modelIdentifier` (e.g. `iPad13,18`), not a person.
    ADD COLUMN device_model text;

-- Console list filter `tracking=` (newest first within an org).
CREATE INDEX sessions_org_tracking_verdict_idx
    ON sessions (org_id, tracking_verdict, started_at DESC)
    WHERE tracking_verdict IS NOT NULL;

-- Review auto-rule `low_tracking_quality` (verdict `poor` or `unusable`).
ALTER TABLE review_items DROP CONSTRAINT review_items_reason_check;
ALTER TABLE review_items ADD CONSTRAINT review_items_reason_check
    CHECK (reason IN ('low_quality', 'flag_red', 'random_audit', 'norm_candidate', 'manual',
                      'low_tracking_quality'));
