-- owner: WP-B.1
-- Initial schema: every table of docs/roadmap_2026q4/02_asd_backend_architecture.md §3
-- plus the WP-B.1 additions (enrol_codes, refresh_tokens, bundle_adoption,
-- sessions.participant_token). Spec ids in comments refer to
-- docs/spec/modules/asd-rust-core-and-backend/technical.md.
--
-- Conventions:
--   * enumerations are text + CHECK (cheap to widen in a later migration);
--   * every timestamp is timestamptz (UTC);
--   * secrets (passwords, tokens, enrol codes) are stored only as hashes; token and
--     code hashes are raw SHA-256 bytes (bytea, 32 bytes);
--   * measurements have no defaults (CON-14): unknown is NULL, never a made-up value;
--   * gen_random_uuid() is built in since Postgres 13 (no extension needed).
-- Application-role grants (audit_log append-only, AUTH-5) are WP-B.2 (0002_auth_grants.sql).

-- ---------------------------------------------------------------- tenancy and people

CREATE TABLE organizations (
    id               uuid        PRIMARY KEY DEFAULT gen_random_uuid(),
    name             text        NOT NULL,
    kind             text        NOT NULL CHECK (kind IN ('home', 'clinic', 'research')),
    created_at       timestamptz NOT NULL DEFAULT now(),
    retention_months integer     NOT NULL DEFAULT 24 CHECK (retention_months > 0),
    settings         jsonb       NOT NULL DEFAULT '{}'::jsonb
);

CREATE TABLE users (
    id            uuid        PRIMARY KEY DEFAULT gen_random_uuid(),
    -- NULL only for the global superadmin role (AUTH-4).
    org_id        uuid        REFERENCES organizations (id),
    email         text        NOT NULL,
    pw_hash       text        NOT NULL,
    totp_secret   text,
    role          text        NOT NULL CHECK (role IN
                      ('superadmin', 'owner', 'admin', 'clinician', 'researcher', 'reviewer', 'viewer')),
    status        text        NOT NULL DEFAULT 'active' CHECK (status IN ('invited', 'active', 'disabled')),
    created_at    timestamptz NOT NULL DEFAULT now(),
    last_login_at timestamptz,
    CONSTRAINT users_org_required CHECK (role = 'superadmin' OR org_id IS NOT NULL)
);
CREATE UNIQUE INDEX users_email_key ON users (lower(email));
CREATE INDEX users_org_id_idx ON users (org_id);

-- AUTH-2: opaque refresh tokens, SHA-256 at rest, rotated on every use; reuse of a
-- rotated-out token revokes the whole family.
CREATE TABLE refresh_tokens (
    id         uuid        PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id    uuid        NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    family_id  uuid        NOT NULL,
    token_hash bytea       NOT NULL UNIQUE CHECK (octet_length(token_hash) = 32),
    created_at timestamptz NOT NULL DEFAULT now(),
    expires_at timestamptz NOT NULL,
    rotated_at timestamptz,
    revoked_at timestamptz
);
CREATE INDEX refresh_tokens_family_idx ON refresh_tokens (family_id);
CREATE INDEX refresh_tokens_user_idx ON refresh_tokens (user_id);

-- CON-11: codes have max_uses (default 1) and expires_at (default 7 days).
CREATE TABLE enrol_codes (
    id         uuid        PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id     uuid        NOT NULL REFERENCES organizations (id),
    code_hash  bytea       NOT NULL UNIQUE CHECK (octet_length(code_hash) = 32),
    max_uses   integer     NOT NULL DEFAULT 1 CHECK (max_uses >= 1),
    uses       integer     NOT NULL DEFAULT 0,
    expires_at timestamptz NOT NULL DEFAULT (now() + interval '7 days'),
    created_by uuid        REFERENCES users (id),
    created_at timestamptz NOT NULL DEFAULT now(),
    revoked_at timestamptz,
    CONSTRAINT enrol_codes_uses_range CHECK (uses >= 0 AND uses <= max_uses)
);
CREATE INDEX enrol_codes_org_idx ON enrol_codes (org_id);

-- AUTH-3: device tokens `dvc_...`, SHA-256 at rest, bound to (device, org), revocable.
CREATE TABLE devices (
    id            uuid        PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id        uuid        NOT NULL REFERENCES organizations (id),
    platform      text        NOT NULL,
    app_version   text        NOT NULL,
    attest_key_id text,
    token_hash    bytea       NOT NULL UNIQUE CHECK (octet_length(token_hash) = 32),
    label         text,
    -- The user who issued the enrolment (NULL for App Attest or system enrolment).
    enrolled_by   uuid        REFERENCES users (id),
    -- CON-11: re-registration with the same code + device_id rotates this device's token.
    enrol_code_id uuid        REFERENCES enrol_codes (id),
    enrolled_at   timestamptz NOT NULL DEFAULT now(),
    revoked_at    timestamptz,
    CONSTRAINT devices_id_org_key UNIQUE (id, org_id)
);
CREATE INDEX devices_org_idx ON devices (org_id);

-- Pseudonymous children: `token` is the app's participantToken; never a name (ADR-B8).
CREATE TABLE participants (
    id                   uuid        PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id               uuid        NOT NULL REFERENCES organizations (id),
    token                text        NOT NULL,
    age_bracket_at_enrol text,
    language             text,
    consent_version      text,
    consent_at           timestamptz,
    mode                 text        CHECK (mode IN ('parent', 'clinic')),
    created_at           timestamptz NOT NULL DEFAULT now(),
    withdrawn_at         timestamptz,
    CONSTRAINT participants_org_token_key UNIQUE (org_id, token),
    CONSTRAINT participants_id_org_key UNIQUE (id, org_id)
);

-- ---------------------------------------------------------------- configuration (CFG)

CREATE TABLE config_documents (
    id           uuid        PRIMARY KEY DEFAULT gen_random_uuid(),
    kind         text        NOT NULL CHECK (kind IN ('stimulus_catalog', 'normative_baselines', 'scoring_profile')),
    version      text        NOT NULL,
    status       text        NOT NULL DEFAULT 'draft'
                     CHECK (status IN ('draft', 'candidate', 'published', 'retired')),
    body         jsonb       NOT NULL,
    signature    text,
    created_by   uuid        REFERENCES users (id),
    created_at   timestamptz NOT NULL DEFAULT now(),
    published_at timestamptz,
    notes        text,
    CONSTRAINT config_documents_kind_version_key UNIQUE (kind, version)
);
CREATE INDEX config_documents_kind_status_idx ON config_documents (kind, status);

-- CFG-1 / CON-6: a signed composition of three published documents. org_id NULL = the
-- global default bundle.
CREATE TABLE config_bundles (
    id             uuid        PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id         uuid        REFERENCES organizations (id),
    catalog_id     uuid        NOT NULL REFERENCES config_documents (id),
    baselines_id   uuid        NOT NULL REFERENCES config_documents (id),
    scoring_id     uuid        NOT NULL REFERENCES config_documents (id),
    bundle_version text        NOT NULL UNIQUE,
    -- The bundle exactly as served (signature included), so GET /config/bundle and its
    -- ETag never re-serialise.
    body           jsonb       NOT NULL,
    signature      text        NOT NULL,
    key_id         text,
    published_at   timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX config_bundles_org_published_idx ON config_bundles (org_id, published_at DESC);

-- CFG-1 adoption metrics: first time each device fetched each bundle version.
CREATE TABLE bundle_adoption (
    device_id      uuid        NOT NULL REFERENCES devices (id) ON DELETE CASCADE,
    bundle_version text        NOT NULL,
    at             timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (device_id, bundle_version)
);
CREATE INDEX bundle_adoption_version_idx ON bundle_adoption (bundle_version);

CREATE TABLE stimulus_assets (
    id                    uuid        PRIMARY KEY DEFAULT gen_random_uuid(),
    clip_id               text        NOT NULL,
    catalog_version       text        NOT NULL,
    file_key              text        NOT NULL,
    duration_sec_measured double precision,
    checksum              text        NOT NULL,
    licence               text        NOT NULL,
    provenance            text        NOT NULL,
    uploaded_by           uuid        REFERENCES users (id),
    created_at            timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT stimulus_assets_clip_version_key UNIQUE (clip_id, catalog_version)
);

-- ---------------------------------------------------------------- sessions (ING)

CREATE TABLE sessions (
    -- sessionId from the app (free text, e.g. a UUID or "SESS-GOLDEN-0001").
    id                     text        PRIMARY KEY,
    org_id                 uuid        NOT NULL REFERENCES organizations (id),
    participant_id         uuid        NOT NULL REFERENCES participants (id),
    -- Denormalised participants.token (console filters and blob keys without a join).
    participant_token      text        NOT NULL,
    device_id              uuid        REFERENCES devices (id),
    -- CON-13: `simulated` is always rejected; `synthetic` only with ASD_ALLOW_SYNTHETIC.
    capture_source         text        NOT NULL CHECK (capture_source IN ('truedepth', 'synthetic')),
    started_at             timestamptz NOT NULL,
    ended_at               timestamptz,
    duration_sec           double precision NOT NULL CHECK (duration_sec >= 0),
    -- CON-16: the catalog protocol id (`express_30s`); NULL when the device sent null.
    battery                text,
    age_pack               text,
    lang                   text,
    bundle_version         text,
    schema_version         text        NOT NULL,
    quality_verdict        text        NOT NULL,
    valid_frame_ratio      double precision NOT NULL CHECK (valid_frame_ratio BETWEEN 0 AND 1),
    calibrated_distance_cm double precision,
    social_preference      double precision NOT NULL,
    eye_dwell              double precision,
    mouth_dwell            double precision,
    first_eye_latency_ms   double precision,
    affect                 jsonb,
    composite_score        double precision,
    composite_confidence   double precision,
    -- CON-15: always derived server-side when the device sent null.
    attention_flag         text        NOT NULL CHECK (attention_flag IN ('green', 'yellow', 'red', 'unreliable')),
    -- The full GazeSessionRecord (rawPayload) as received (ING-2).
    summary                jsonb       NOT NULL,
    frames_blob_key        text,
    report_pdf_key         text,
    received_at            timestamptz NOT NULL DEFAULT now(),
    idempotency_key        text        UNIQUE,
    -- Participant, its denormalised token and the device all belong to the session's org.
    CONSTRAINT sessions_participant_same_org_fk FOREIGN KEY (participant_id, org_id)
        REFERENCES participants (id, org_id),
    CONSTRAINT sessions_participant_token_fk FOREIGN KEY (org_id, participant_token)
        REFERENCES participants (org_id, token) ON UPDATE CASCADE,
    CONSTRAINT sessions_device_same_org_fk FOREIGN KEY (device_id, org_id)
        REFERENCES devices (id, org_id)
);
CREATE INDEX sessions_org_started_idx ON sessions (org_id, started_at DESC, id DESC);
CREATE INDEX sessions_participant_started_idx ON sessions (participant_id, started_at);
CREATE INDEX sessions_org_flag_idx ON sessions (org_id, attention_flag);
CREATE INDEX sessions_summary_gin ON sessions USING gin (summary);

-- One row per perVideo entry (snake_case on the wire, trap 4). Unmeasured values are NULL.
CREATE TABLE session_clip_scores (
    session_id           text        NOT NULL REFERENCES sessions (id) ON DELETE CASCADE,
    clip_id              text        NOT NULL,
    -- perVideo[].order (renamed: ORDER is a reserved word).
    clip_order           integer     NOT NULL,
    dwell_social         double precision,
    dwell_geometric      double precision,
    dwell_eye            double precision,
    dwell_mouth          double precision,
    orienting_latency_ms double precision,
    gaze_cue_accuracy    double precision,
    fixation_count       integer,
    mean_fixation_ms     double precision,
    status               text        CHECK (status IN ('onTarget', 'monitor', 'atypical')),
    score                double precision,
    weight               double precision,
    baseline_ref         text,
    details              jsonb,
    PRIMARY KEY (session_id, clip_id)
);

-- ADR-B5 / CON-17 / trap 7: insert the key first (ON CONFLICT (scope, key) DO NOTHING);
-- zero rows returned = replay, then compare request_hash for 200 vs 409.
CREATE TABLE idempotency_keys (
    scope         text        NOT NULL,
    key           text        NOT NULL,
    -- SHA-256 of the canonical request body.
    request_hash  bytea       NOT NULL CHECK (octet_length(request_hash) = 32),
    -- The original response, replayed with `duplicate: true`.
    status_code   integer,
    response_body jsonb,
    created_at    timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (scope, key)
);
-- TTL sweep (7 days).
CREATE INDEX idempotency_keys_created_idx ON idempotency_keys (created_at);

CREATE TABLE care_events (
    id              uuid        PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id          uuid        NOT NULL REFERENCES organizations (id),
    participant_id  uuid        NOT NULL REFERENCES participants (id),
    kind            text        NOT NULL CHECK (kind IN ('task', 'medication', 'diary', 'milestone')),
    occurred_at     timestamptz NOT NULL,
    payload         jsonb       NOT NULL DEFAULT '{}'::jsonb,
    idempotency_key text,
    created_at      timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT care_events_org_key UNIQUE (org_id, idempotency_key)
);
CREATE INDEX care_events_participant_idx ON care_events (participant_id, occurred_at);

-- ---------------------------------------------------------------- review and labels

-- ING-3 auto-rules; one item per (session, reason).
CREATE TABLE review_items (
    id          uuid        PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id      uuid        NOT NULL REFERENCES organizations (id),
    session_id  text        NOT NULL REFERENCES sessions (id) ON DELETE CASCADE,
    reason      text        NOT NULL CHECK (reason IN ('low_quality', 'flag_red', 'random_audit', 'norm_candidate')),
    status      text        NOT NULL DEFAULT 'open' CHECK (status IN ('open', 'in_review', 'done', 'dismissed')),
    assigned_to uuid        REFERENCES users (id),
    decision    jsonb,
    decided_by  uuid        REFERENCES users (id),
    decided_at  timestamptz,
    created_at  timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT review_items_session_reason_key UNIQUE (session_id, reason)
);
CREATE INDEX review_items_org_status_idx ON review_items (org_id, status);

-- Human annotations for ML.
CREATE TABLE labels (
    id         uuid        PRIMARY KEY DEFAULT gen_random_uuid(),
    session_id text        NOT NULL REFERENCES sessions (id) ON DELETE CASCADE,
    clip_id    text,
    key        text        NOT NULL,
    value      jsonb       NOT NULL,
    created_by uuid        REFERENCES users (id),
    created_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX labels_session_idx ON labels (session_id);

-- ---------------------------------------------------------------- jobs and norms

CREATE TABLE jobs (
    id          uuid        PRIMARY KEY DEFAULT gen_random_uuid(),
    kind        text        NOT NULL,
    status      text        NOT NULL DEFAULT 'queued'
                    CHECK (status IN ('queued', 'running', 'succeeded', 'failed', 'cancelled')),
    params      jsonb       NOT NULL DEFAULT '{}'::jsonb,
    result      jsonb,
    attempts    integer     NOT NULL DEFAULT 0 CHECK (attempts >= 0),
    created_by  uuid        REFERENCES users (id),
    created_at  timestamptz NOT NULL DEFAULT now(),
    started_at  timestamptz,
    finished_at timestamptz,
    error       text
);
-- Worker poll: FOR UPDATE SKIP LOCKED over queued jobs, oldest first.
CREATE INDEX jobs_queued_idx ON jobs (created_at) WHERE status = 'queued';

CREATE TABLE norm_runs (
    id                    uuid        PRIMARY KEY DEFAULT gen_random_uuid(),
    baselines_source_id   uuid        REFERENCES config_documents (id),
    job_id                uuid        REFERENCES jobs (id),
    params                jsonb       NOT NULL,
    cells                 jsonb       NOT NULL,
    n_total               integer     NOT NULL CHECK (n_total >= 0),
    created_by            uuid        REFERENCES users (id),
    created_at            timestamptz NOT NULL DEFAULT now(),
    promoted_to_config_id uuid        REFERENCES config_documents (id)
);

-- ---------------------------------------------------------------- privacy and audit

CREATE TABLE erasure_requests (
    id                uuid        PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id            uuid        NOT NULL REFERENCES organizations (id),
    participant_id    uuid        NOT NULL REFERENCES participants (id),
    requested_by_type text        NOT NULL CHECK (requested_by_type IN ('user', 'device', 'system')),
    requested_by      uuid,
    requested_at      timestamptz NOT NULL DEFAULT now(),
    completed_at      timestamptz,
    scope             text        NOT NULL DEFAULT 'all'
);
CREATE INDEX erasure_requests_open_idx ON erasure_requests (requested_at) WHERE completed_at IS NULL;

-- AUTH-5: append-only. No foreign keys, so audit rows outlive what they describe.
CREATE TABLE audit_log (
    id          bigint      GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    ts          timestamptz NOT NULL DEFAULT now(),
    actor_type  text        NOT NULL CHECK (actor_type IN ('user', 'device', 'system')),
    actor_id    text,
    org_id      uuid,
    action      text        NOT NULL,
    target_type text        NOT NULL,
    target_id   text,
    before      jsonb,
    after       jsonb,
    ip          inet,
    user_agent  text
);
CREATE INDEX audit_log_org_ts_idx ON audit_log (org_id, ts);
