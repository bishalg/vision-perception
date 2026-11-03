-- owner: WP-X.0
-- Researcher toolkit (ADR-0012; docs/spec/modules/asd-research-toolkit/technical.md,
-- "Migration 0026"): experiments, cohorts and analysis runs (EXP-1, COH-2, RUN-1), the
-- research frame log columns of a session (RFL-3, RFL-5), the session's age as a column
-- (COH-3), job progress and cancellation (RUN-8) and the `analysis_results` export format
-- (RUN-7). Nothing here is filled by this package beyond the `age_months` back-fill; the
-- routes and the job come with WP-X.2 / WP-X.3 / WP-X.8. B.21 (identity) is 0027.

-- ---------------------------------------------------------------- sessions (RFL, COH-3)

-- `age_months`: the device's `summary.ageMonths`, set by ingest from WP-X.2 on, used by the
-- cohort band filters. `research_frames_*`: the sealed research frame log (RFL-3), purged by
-- the retention sweep (RFL-5) like `frames_blob_key`.
ALTER TABLE sessions
    ADD COLUMN age_months                int         CHECK (age_months BETWEEN 0 AND 240),
    ADD COLUMN research_frames_blob_key  text,
    ADD COLUMN research_frames_bytes     bigint      CHECK (research_frames_bytes >= 0),
    ADD COLUMN research_frames_sha256    bytea       CHECK (octet_length(research_frames_sha256) = 32),
    ADD COLUMN research_frames_purged_at timestamptz,
    ADD CONSTRAINT sessions_research_frames_complete CHECK (
        (research_frames_blob_key IS NULL) = (research_frames_sha256 IS NULL)
        AND (research_frames_blob_key IS NULL) = (research_frames_bytes IS NULL));

-- Back-fill from the stored payload: whole numbers 0..240 only; anything else (a float, a
-- string, a negative or absurd number) stays NULL. The CASE keeps the cast behind the
-- pattern check (a WHERE clause does not fix the evaluation order).
UPDATE sessions
SET age_months = (summary ->> 'ageMonths')::int
WHERE summary ? 'ageMonths'
  AND CASE WHEN (summary ->> 'ageMonths') ~ '^[0-9]{1,3}$'
           THEN (summary ->> 'ageMonths')::int <= 240
           ELSE false END;

-- Cohort membership scans an org's sessions by arrival (COH-1 `from`/`to`): served by
-- `sessions_org_received_idx (org_id, received_at DESC, id DESC)`, which 0015 already
-- created. The cohort preview counts the sessions that have a research log (RFL-6). Consent
-- lives in JSONB, so no consent-filtered index is possible.
CREATE INDEX sessions_research_frames_idx ON sessions (org_id)
    WHERE research_frames_blob_key IS NOT NULL;

-- ---------------------------------------------------------------- organizations (RFL-5)

-- NULL: research logs follow `retention_months` (ADR-0012 Q1 default).
ALTER TABLE organizations
    ADD COLUMN research_frames_retention_months int
        CHECK (research_frames_retention_months BETWEEN 1 AND 240);

-- ---------------------------------------------------------------- jobs (RUN-8)

-- `progress`: `{done, total, phase}`, written with the heartbeat (`JobContext::progress`).
-- `cancel_requested_at`: set by `POST /v1/jobs/{id}/cancel`; handlers check it between pages.
ALTER TABLE jobs
    ADD COLUMN progress            jsonb CHECK (progress IS NULL OR jsonb_typeof(progress) = 'object'),
    ADD COLUMN cancel_requested_at timestamptz;

-- ---------------------------------------------------------------- export format (RUN-7)

ALTER TABLE exports DROP CONSTRAINT exports_format_check;
ALTER TABLE exports ADD CONSTRAINT exports_format_check
    CHECK (format IN ('csv', 'csv_excel', 'csv_sheets', 'xlsx', 'pdf_summary', 'fhir_bundle',
                      'fhir_ndjson', 'hl7v2', 'dataset_jsonl', 'dataset_parquet',
                      'case_package', 'audit_csv', 'analysis_results'));

-- ---------------------------------------------------------------- experiments (EXP-1)

CREATE TABLE experiments (
    id                      uuid        PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id                  uuid        NOT NULL REFERENCES organizations (id),
    name                    text        NOT NULL CHECK (char_length(name) BETWEEN 1 AND 80),
    description             text        CHECK (char_length(description) <= 500),
    status                  text        NOT NULL DEFAULT 'draft'
                                CHECK (status IN ('draft', 'archived', 'promoted')),
    base_profile_version    text        NOT NULL,
    -- The full ScoringProfile (schema 1.2.0) and sha256 of its RFC 8785 form.
    profile                 jsonb       NOT NULL CHECK (jsonb_typeof(profile) = 'object'),
    profile_hash            bytea       NOT NULL CHECK (octet_length(profile_hash) = 32),
    baselines_version       text,
    promoted_config_version text,
    last_run_id             uuid,
    -- If-Match on PATCH (EXP-2, SIGN-3).
    row_version             integer     NOT NULL DEFAULT 1 CHECK (row_version >= 1),
    created_by              uuid        NOT NULL REFERENCES users (id),
    created_at              timestamptz NOT NULL DEFAULT now(),
    updated_at              timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT experiments_org_name_key UNIQUE (org_id, name),
    CONSTRAINT experiments_id_org_key UNIQUE (id, org_id),
    CONSTRAINT experiments_promoted_version CHECK (
        status <> 'promoted' OR promoted_config_version IS NOT NULL)
);
CREATE INDEX experiments_org_updated_idx ON experiments (org_id, updated_at DESC, id DESC);

-- ---------------------------------------------------------------- cohorts (COH-2)

CREATE TABLE cohorts (
    id              uuid        PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id          uuid        NOT NULL REFERENCES organizations (id),
    name            text        NOT NULL CHECK (char_length(name) BETWEEN 1 AND 80),
    -- A CohortDefinition and sha256 of its RFC 8785 form (`cohort_hash`).
    definition      jsonb       NOT NULL CHECK (jsonb_typeof(definition) = 'object'),
    definition_hash bytea       NOT NULL CHECK (octet_length(definition_hash) = 32),
    created_by      uuid        NOT NULL REFERENCES users (id),
    created_at      timestamptz NOT NULL DEFAULT now(),
    updated_at      timestamptz NOT NULL DEFAULT now(),
    archived_at     timestamptz,
    CONSTRAINT cohorts_org_name_key UNIQUE (org_id, name),
    CONSTRAINT cohorts_id_org_key UNIQUE (id, org_id)
);
CREATE INDEX cohorts_org_updated_idx ON cohorts (org_id, updated_at DESC, id DESC);

-- ---------------------------------------------------------------- analysis runs (RUN-1)

-- Every column from `definition_hash` to `seed` is set before the job starts (RUN-5), so
-- they are NOT NULL (except `membership_hash`, set by the job's first phase, and
-- `bundle_version`). The experiment and cohort must be of the run's org (composite keys).
-- Per-session results live in the sealed blob `result_blob_key` (org DEK), never here.
CREATE TABLE analysis_runs (
    id                        uuid        PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id                    uuid        NOT NULL REFERENCES organizations (id),
    kind                      text        NOT NULL
                                  CHECK (kind IN ('cohort_rescore', 'param_sweep', 'age_profile')),
    status                    text        NOT NULL DEFAULT 'queued'
                                  CHECK (status IN ('queued', 'running', 'succeeded', 'failed',
                                                    'cancelled')),
    experiment_id             uuid,
    cohort_id                 uuid,
    definition                jsonb       NOT NULL CHECK (jsonb_typeof(definition) = 'object'),
    definition_hash           bytea       NOT NULL CHECK (octet_length(definition_hash) = 32),
    cohort_hash               bytea       NOT NULL CHECK (octet_length(cohort_hash) = 32),
    membership_hash           bytea       CHECK (octet_length(membership_hash) = 32),
    profile_hash              bytea       NOT NULL CHECK (octet_length(profile_hash) = 32),
    published_profile_version text        NOT NULL,
    baselines_version         text        NOT NULL,
    catalog_version           text        NOT NULL,
    bundle_version            text,
    core_version              text        NOT NULL,
    contract_version          text        NOT NULL,
    pipeline_version          text        NOT NULL,
    registry_version          text        NOT NULL,
    seed                      bigint      NOT NULL,
    level_requested           text        NOT NULL CHECK (level_requested IN ('auto', 'score', 'frames')),
    sessions_n                integer     CHECK (sessions_n >= 0),
    sessions_frames_n         integer     CHECK (sessions_frames_n >= 0),
    sessions_score_n          integer     CHECK (sessions_score_n >= 0),
    excluded                  jsonb,
    summary                   jsonb,
    result_blob_key           text,
    result_bytes              integer     CHECK (result_bytes >= 0),
    progress                  jsonb,
    job_id                    uuid        REFERENCES jobs (id),
    error                     text,
    created_by                uuid        REFERENCES users (id),
    created_at                timestamptz NOT NULL DEFAULT now(),
    started_at                timestamptz,
    finished_at               timestamptz,
    CONSTRAINT analysis_runs_experiment_same_org_fk FOREIGN KEY (experiment_id, org_id)
        REFERENCES experiments (id, org_id),
    CONSTRAINT analysis_runs_cohort_same_org_fk FOREIGN KEY (cohort_id, org_id)
        REFERENCES cohorts (id, org_id),
    CONSTRAINT analysis_runs_result_blob CHECK ((result_blob_key IS NULL) = (result_bytes IS NULL))
);
CREATE INDEX analysis_runs_org_created_idx ON analysis_runs (org_id, created_at DESC, id DESC);
-- RUN-9 quota: queued and running runs of an org.
CREATE INDEX analysis_runs_org_active_idx ON analysis_runs (org_id)
    WHERE status IN ('queued', 'running');
CREATE INDEX analysis_runs_experiment_idx ON analysis_runs (experiment_id)
    WHERE experiment_id IS NOT NULL;

-- GOV-2: promote needs the experiment's latest run; deleting a run (never done today)
-- unlinks it.
ALTER TABLE experiments
    ADD CONSTRAINT experiments_last_run_fk FOREIGN KEY (last_run_id)
        REFERENCES analysis_runs (id) ON DELETE SET NULL;
