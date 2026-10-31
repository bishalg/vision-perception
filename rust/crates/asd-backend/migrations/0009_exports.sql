-- owner: WP-I.6
-- Export jobs, artifacts, disclosures and delivery bookkeeping (MIG-3, EXJ-1..9, INTG-1,
-- DLV-1/5; docs/spec/modules/asd-clinician-and-interop/technical.md, ADR-0008). Migration
-- 0007 stays unused forever (MIG-1); 0010 is reserved for WP-I.9 (MIG-4).
--
-- Nothing here holds clinical content: an export row names its filters, counts and the
-- artifact's hash; the artifact itself lives in the blob store under
-- `exports/{org_id}/{export_id}/{file}` until `expires_at` (EXJ-4). `export_items` is the
-- accounting of disclosures (EXJ-6) and outlives the artifact.

-- ---------------------------------------------------------------- job runner (G5)

-- `run_after`: a queued job is not claimed before this time (delayed retry, DLV-1 backoff).
-- `heartbeat_at`: refreshed by a running phased job (exports), so the visibility-timeout
-- reclaim does not hand a long export to a second worker (EXJ-9, trap 4).
ALTER TABLE jobs ADD COLUMN run_after timestamptz NOT NULL DEFAULT now();
ALTER TABLE jobs ADD COLUMN heartbeat_at timestamptz;
DROP INDEX jobs_queued_idx;
CREATE INDEX jobs_queued_idx ON jobs (run_after, created_at) WHERE status = 'queued';

-- ---------------------------------------------------------------- exports (EXJ)

CREATE TABLE exports (
    id                 uuid        PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id             uuid        NOT NULL REFERENCES organizations (id),
    -- `bulk`: a job renders and stores an artifact; `document`: a single synchronous render
    -- (EXJ-7, NOTE-4/5), streamed and never stored.
    kind               text        NOT NULL DEFAULT 'bulk' CHECK (kind IN ('bulk', 'document')),
    format             text        NOT NULL CHECK (format IN ('csv', 'csv_excel', 'csv_sheets',
                           'xlsx', 'pdf_summary', 'fhir_bundle', 'fhir_ndjson', 'hl7v2')),
    mode               text        NOT NULL CHECK (mode IN ('identified', 'deidentified')),
    purpose            text        NOT NULL CHECK (purpose IN ('clinical_care', 'research',
                           'quality_improvement', 'patient_request')),
    status             text        NOT NULL DEFAULT 'queued'
                           CHECK (status IN ('queued', 'running', 'succeeded', 'failed', 'expired')),
    -- The request's filters as sent (EXJ-1); never MRNs or free text.
    filters            jsonb       NOT NULL DEFAULT '{}'::jsonb,
    reason             text        NOT NULL CHECK (char_length(reason) BETWEEN 10 AND 500),
    destination_ids    uuid[]      NOT NULL DEFAULT '{}',
    -- What the requester could read when they asked (EXJ-2, MRN-6): the job never widens it.
    include_clinical_content boolean NOT NULL DEFAULT false,
    include_mrn        boolean     NOT NULL DEFAULT false,
    requested_by       uuid        REFERENCES users (id),
    job_id             uuid        REFERENCES jobs (id),
    ttl_hours          integer     NOT NULL DEFAULT 72 CHECK (ttl_hours BETWEEN 1 AND 168),
    -- Per-export HMAC key of proxy-mode download tokens (EXJ-5); never returned.
    download_secret    bytea       NOT NULL CHECK (octet_length(download_secret) = 32),
    blob_key           text,
    file_name          text,
    content_type       text,
    sha256             bytea       CHECK (octet_length(sha256) = 32),
    byte_size          bigint      CHECK (byte_size >= 0),
    row_count          integer     CHECK (row_count >= 0),
    -- Excluded sessions by reason (`withdrawn`, `no_research_consent`, ...; EXJ-3).
    excluded           jsonb       NOT NULL DEFAULT '{}'::jsonb,
    -- A short error class for the console; never PHI or remote text.
    error              text        CHECK (char_length(error) <= 500),
    created_at         timestamptz NOT NULL DEFAULT now(),
    started_at         timestamptz,
    completed_at       timestamptz,
    expires_at         timestamptz,
    expired_at         timestamptz,
    CONSTRAINT exports_id_org_key UNIQUE (id, org_id),
    CONSTRAINT exports_succeeded_has_artifact CHECK (
        status <> 'succeeded' OR kind = 'document'
        OR (blob_key IS NOT NULL AND sha256 IS NOT NULL AND byte_size IS NOT NULL
            AND expires_at IS NOT NULL))
);
CREATE INDEX exports_org_created_idx ON exports (org_id, created_at DESC, id DESC);
-- The expiry sweep (EXJ-4).
CREATE INDEX exports_expiry_idx ON exports (expires_at) WHERE status = 'succeeded';

-- One row per session included in an export (EXJ-3, EXJ-6). No foreign key to sessions or
-- participants: the accounting of disclosures must survive erasure (EXJ-8).
CREATE TABLE export_items (
    export_id      uuid NOT NULL REFERENCES exports (id),
    participant_id uuid NOT NULL,
    session_id     text NOT NULL,
    PRIMARY KEY (export_id, session_id)
);
CREATE INDEX export_items_participant_idx ON export_items (participant_id);

-- ---------------------------------------------------------------- integrations (INTG, DLV)

-- Destinations per org (INTG-1). Settings live here, never in `organizations.settings`
-- (G3: that column's history is copied into the append-only audit log). The secret is
-- write-only and encrypted (KEY-1, AAD "integration_destinations:secret_ct:{org}:{id}").
CREATE TABLE integration_destinations (
    id                 uuid        PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id             uuid        NOT NULL REFERENCES organizations (id),
    kind               text        NOT NULL CHECK (kind IN ('webhook', 'sftp', 'fhir_server', 'hl7_mllp')),
    name               text        NOT NULL CHECK (char_length(name) BETWEEN 1 AND 120),
    config             jsonb       NOT NULL DEFAULT '{}'::jsonb,
    secret_ct          bytea,
    secret_key_id      uuid,
    secret_fingerprint text        CHECK (secret_fingerprint ~ '^[0-9a-f]{8}$'),
    status             text        NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'disabled')),
    environment        text        NOT NULL DEFAULT 'test' CHECK (environment IN ('test', 'production')),
    created_by         uuid        REFERENCES users (id),
    created_at         timestamptz NOT NULL DEFAULT now(),
    updated_at         timestamptz NOT NULL DEFAULT now(),
    last_test_at       timestamptz,
    last_test_ok       boolean,
    CONSTRAINT integration_destinations_id_org_key UNIQUE (id, org_id),
    CONSTRAINT integration_destinations_secret_key
        CHECK ((secret_ct IS NULL) = (secret_key_id IS NULL))
);
CREATE INDEX integration_destinations_org_idx ON integration_destinations (org_id, created_at);

-- One row per (export, destination) (DLV-1, DLV-5).
CREATE TABLE export_deliveries (
    id               uuid        PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id           uuid        NOT NULL REFERENCES organizations (id),
    export_id        uuid        NOT NULL,
    destination_id   uuid        NOT NULL,
    status           text        NOT NULL DEFAULT 'pending'
                         CHECK (status IN ('pending', 'sending', 'delivered', 'failed')),
    attempts         integer     NOT NULL DEFAULT 0 CHECK (attempts >= 0),
    last_error_class text        CHECK (char_length(last_error_class) <= 64),
    remote_ref       text        CHECK (char_length(remote_ref) <= 200),
    job_id           uuid        REFERENCES jobs (id),
    created_at       timestamptz NOT NULL DEFAULT now(),
    updated_at       timestamptz NOT NULL DEFAULT now(),
    delivered_at     timestamptz,
    CONSTRAINT export_deliveries_pair_key UNIQUE (export_id, destination_id),
    CONSTRAINT export_deliveries_export_same_org_fk FOREIGN KEY (export_id, org_id)
        REFERENCES exports (id, org_id),
    CONSTRAINT export_deliveries_destination_same_org_fk FOREIGN KEY (destination_id, org_id)
        REFERENCES integration_destinations (id, org_id)
);
CREATE INDEX export_deliveries_destination_idx ON export_deliveries (destination_id, created_at DESC);
