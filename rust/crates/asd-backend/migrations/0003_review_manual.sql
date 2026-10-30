-- owner: WP-B.3
-- Review queue additions for `POST /v1/orgs/{id}/review` (API-2): the contract's
-- `ReviewReason` has `manual` beside the four ING-3 auto-rules, and a manual enqueue may
-- carry a short free-text note. Notes are never logged (OBS-1 drops `notes`) and are not
-- returned by `ReviewItem`.

ALTER TABLE review_items DROP CONSTRAINT review_items_reason_check;
ALTER TABLE review_items ADD CONSTRAINT review_items_reason_check
    CHECK (reason IN ('low_quality', 'flag_red', 'random_audit', 'norm_candidate', 'manual'));

ALTER TABLE review_items ADD COLUMN note text;
ALTER TABLE review_items ADD COLUMN created_by uuid REFERENCES users (id);

-- Console queue ordering (newest first) and the per-session "reviewed" lookup.
CREATE INDEX review_items_org_created_idx ON review_items (org_id, created_at DESC, id DESC);
