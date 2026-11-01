-- owner: WP-B.9
-- Dataset export, retention sweep and erasure execution (02 §4/§5, ADR-B8; EXJ-8 of
-- docs/spec/modules/asd-clinician-and-interop/technical.md). Numbered 0017; 0018 belongs to
-- a package that ran in parallel (sqlx applies a lower number added later as long as it was
-- never applied).

-- ---------------------------------------------------------------- dataset exports

-- ML datasets are exports (same job, consent filter, de-identification, artifact store,
-- signed download, expiry and erasure handling); two new formats.
ALTER TABLE exports DROP CONSTRAINT exports_format_check;
ALTER TABLE exports ADD CONSTRAINT exports_format_check
    CHECK (format IN ('csv', 'csv_excel', 'csv_sheets', 'xlsx', 'pdf_summary', 'fhir_bundle',
                      'fhir_ndjson', 'hl7v2', 'dataset_jsonl', 'dataset_parquet'));
-- Dataset formats: the artifact carries the sessions' reviewer labels.
ALTER TABLE exports ADD COLUMN include_labels boolean NOT NULL DEFAULT false;
-- Datasets are de-identified only (XC-4); the API refuses, this makes sure.
ALTER TABLE exports ADD CONSTRAINT exports_dataset_deidentified
    CHECK (format NOT IN ('dataset_jsonl', 'dataset_parquet') OR mode = 'deidentified');
-- Erasure finds the unexpired artifacts that included a participant (EXJ-8).
CREATE INDEX IF NOT EXISTS export_items_export_participant_idx
    ON export_items (participant_id, export_id);

-- ---------------------------------------------------------------- erasure execution

-- `status`: pending -> completed | held (see asd_contract::privacy::ErasureStatus).
-- `completed_at` stays NULL while held, so the one-open-request index keeps refusing a
-- second request for the participant.
ALTER TABLE erasure_requests
    ADD COLUMN status text NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending', 'completed', 'held')),
    ADD COLUMN job_id uuid REFERENCES jobs (id),
    ADD COLUMN executed_at timestamptz,
    ADD COLUMN hold_reason text CHECK (hold_reason IN ('clinical_record')),
    -- Counts only (asd_contract::privacy::ErasureOutcome), never identifiers.
    ADD COLUMN outcome jsonb;
UPDATE erasure_requests SET status = 'completed', executed_at = completed_at
    WHERE completed_at IS NOT NULL;
ALTER TABLE erasure_requests
    ADD CONSTRAINT erasure_requests_completed_status
        CHECK ((status = 'completed') = (completed_at IS NOT NULL)),
    ADD CONSTRAINT erasure_requests_hold_reason
        CHECK ((status = 'held') = (hold_reason IS NOT NULL));
CREATE INDEX erasure_requests_org_requested_idx
    ON erasure_requests (org_id, requested_at DESC, id DESC);
CREATE INDEX erasure_requests_pending_idx ON erasure_requests (requested_at)
    WHERE status = 'pending';

-- The tombstone: an erased participant keeps its row (id, org, token, withdrawn_at) so a
-- replayed upload for the token stays refused (`participant-withdrawn`), and erasure
-- requests and the accounting of disclosures keep their reference. Demographics are
-- cleared when `erased_at` is set.
ALTER TABLE participants ADD COLUMN erased_at timestamptz;
ALTER TABLE participants ADD CONSTRAINT participants_erased_is_withdrawn
    CHECK (erased_at IS NULL OR withdrawn_at IS NOT NULL);

-- ---------------------------------------------------------------- retention sweep

-- Set when the retention sweep deleted the session's frames blob (02 §5: frames beyond
-- retention are deleted, the session summary and its aggregate counts stay).
ALTER TABLE sessions ADD COLUMN frames_purged_at timestamptz;
CREATE INDEX sessions_org_retention_idx ON sessions (org_id, received_at)
    WHERE frames_purged_at IS NULL;
