-- owner: WP-X.11
-- Research datasets (ADR-0015 R-7..R-9; docs/spec/modules/asd-research-toolkit/datasets.md):
-- a frozen, hashed member list taken from a cohort, its exclusions, labels, split and lineage,
-- and the run columns that name the dataset a run read. 0027 stays reserved for WP-B.21
-- (identity); if B.21 lands after this file it takes the next free number above the highest.
--
-- Immutability: a dataset row never changes after the freeze except `status`/`archived_at`
-- (archive) and the erasure counter; a member row never changes except the erasure marking,
-- which clears its data and keeps only the session id (R-9). Triggers enforce both, so a bug
-- in a route cannot rewrite a frozen dataset.

-- ---------------------------------------------------------------- datasets (R-7)

CREATE TABLE datasets (
    id                uuid        PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id            uuid        NOT NULL REFERENCES organizations (id),
    name              text        NOT NULL CHECK (char_length(name) BETWEEN 1 AND 80),
    description       text        CHECK (char_length(description) <= 500),
    status            text        NOT NULL DEFAULT 'frozen' CHECK (status IN ('frozen', 'archived')),
    cohort_id         uuid,
    -- The cohort definition as used at freeze time (the cohort may change later).
    cohort_definition jsonb       NOT NULL CHECK (jsonb_typeof(cohort_definition) = 'object'),
    parent_dataset_id uuid,
    run_id            uuid,
    frozen_at         timestamptz NOT NULL DEFAULT now(),
    -- asd_contract::datasets::content_hash (sha256 of the RFC 8785 hash document).
    content_hash      bytea       NOT NULL CHECK (octet_length(content_hash) = 32),
    member_count      integer     NOT NULL CHECK (member_count >= 0),
    erased_count      integer     NOT NULL DEFAULT 0 CHECK (erased_count >= 0),
    exclusion_rules   jsonb       NOT NULL CHECK (jsonb_typeof(exclusion_rules) = 'array'),
    exclusions        jsonb       NOT NULL CHECK (jsonb_typeof(exclusions) = 'array'),
    label_source      jsonb       CHECK (label_source IS NULL OR jsonb_typeof(label_source) = 'object'),
    split             jsonb       CHECK (split IS NULL OR jsonb_typeof(split) = 'object'),
    split_counts      jsonb       NOT NULL DEFAULT '[]' CHECK (jsonb_typeof(split_counts) = 'array'),
    lineage           jsonb       NOT NULL CHECK (jsonb_typeof(lineage) = 'object'),
    data_dictionary   jsonb       NOT NULL CHECK (jsonb_typeof(data_dictionary) = 'array'),
    created_by        uuid        NOT NULL REFERENCES users (id),
    created_at        timestamptz NOT NULL DEFAULT now(),
    archived_at       timestamptz,
    CONSTRAINT datasets_org_name_key UNIQUE (org_id, name),
    CONSTRAINT datasets_id_org_key UNIQUE (id, org_id),
    CONSTRAINT datasets_cohort_same_org_fk FOREIGN KEY (cohort_id, org_id)
        REFERENCES cohorts (id, org_id),
    CONSTRAINT datasets_parent_same_org_fk FOREIGN KEY (parent_dataset_id, org_id)
        REFERENCES datasets (id, org_id),
    CONSTRAINT datasets_run_fk FOREIGN KEY (run_id) REFERENCES analysis_runs (id),
    CONSTRAINT datasets_archived CHECK ((status = 'archived') = (archived_at IS NOT NULL))
);
CREATE INDEX datasets_org_created_idx ON datasets (org_id, created_at DESC, id DESC);

-- Members: one row per session of the cohort snapshot that is IN the dataset (exclusions
-- live in `datasets.exclusions`). `session_id` has no foreign key: an erased session's row
-- stays, marked (R-9).
CREATE TABLE dataset_members (
    dataset_id            uuid        NOT NULL REFERENCES datasets (id),
    session_id            text        NOT NULL,
    participant_pseudonym text,
    age_months            integer     CHECK (age_months BETWEEN 0 AND 240),
    age_band              text,
    split                 text        CHECK (split IN ('train', 'validation', 'holdout')),
    -- asd_contract::datasets::Label.
    label                 jsonb       CHECK (label IS NULL OR jsonb_typeof(label) = 'object'),
    erased_at             timestamptz,
    PRIMARY KEY (dataset_id, session_id),
    CONSTRAINT dataset_members_erased_clears CHECK (
        erased_at IS NULL OR (participant_pseudonym IS NULL AND age_months IS NULL
                              AND age_band IS NULL AND split IS NULL AND label IS NULL))
);
-- The erasure job finds a session's memberships.
CREATE INDEX dataset_members_session_idx ON dataset_members (session_id);

-- ---------------------------------------------------------------- immutability (R-7)

CREATE FUNCTION datasets_frozen() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.id <> OLD.id OR NEW.org_id <> OLD.org_id OR NEW.name <> OLD.name
       OR NEW.description IS DISTINCT FROM OLD.description
       OR NEW.cohort_id IS DISTINCT FROM OLD.cohort_id
       OR NEW.cohort_definition <> OLD.cohort_definition
       OR NEW.parent_dataset_id IS DISTINCT FROM OLD.parent_dataset_id
       OR NEW.run_id IS DISTINCT FROM OLD.run_id
       OR NEW.frozen_at <> OLD.frozen_at OR NEW.content_hash <> OLD.content_hash
       OR NEW.member_count <> OLD.member_count
       OR NEW.exclusion_rules <> OLD.exclusion_rules OR NEW.exclusions <> OLD.exclusions
       OR NEW.label_source IS DISTINCT FROM OLD.label_source
       OR NEW.split IS DISTINCT FROM OLD.split OR NEW.split_counts <> OLD.split_counts
       OR NEW.lineage <> OLD.lineage OR NEW.data_dictionary <> OLD.data_dictionary
       OR NEW.created_by <> OLD.created_by OR NEW.created_at <> OLD.created_at
       OR NEW.erased_count < OLD.erased_count
       OR (OLD.status = 'archived' AND NEW.status <> 'archived') THEN
        RAISE EXCEPTION 'dataset % is frozen', OLD.id USING ERRCODE = 'check_violation';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER datasets_frozen BEFORE UPDATE ON datasets
    FOR EACH ROW EXECUTE FUNCTION datasets_frozen();

CREATE FUNCTION dataset_members_frozen() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP = 'DELETE' THEN
        RAISE EXCEPTION 'dataset members are frozen' USING ERRCODE = 'check_violation';
    END IF;
    -- The one allowed change: the erasure marking (the CHECK clears the data with it).
    IF NEW.dataset_id <> OLD.dataset_id OR NEW.session_id <> OLD.session_id
       OR OLD.erased_at IS NOT NULL OR NEW.erased_at IS NULL THEN
        RAISE EXCEPTION 'dataset members are frozen' USING ERRCODE = 'check_violation';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER dataset_members_frozen BEFORE UPDATE OR DELETE ON dataset_members
    FOR EACH ROW EXECUTE FUNCTION dataset_members_frozen();

-- ---------------------------------------------------------------- analysis runs

-- A run started from a dataset reads its members, not the cohort (WP-X.11); the hash is the
-- dataset's content hash at enqueue time (provenance, RUN-5).
ALTER TABLE analysis_runs
    ADD COLUMN dataset_id   uuid,
    ADD COLUMN dataset_hash bytea CHECK (octet_length(dataset_hash) = 32),
    ADD CONSTRAINT analysis_runs_dataset_same_org_fk FOREIGN KEY (dataset_id, org_id)
        REFERENCES datasets (id, org_id),
    ADD CONSTRAINT analysis_runs_dataset_hash CHECK ((dataset_id IS NULL) = (dataset_hash IS NULL));
CREATE INDEX analysis_runs_dataset_idx ON analysis_runs (dataset_id) WHERE dataset_id IS NOT NULL;
