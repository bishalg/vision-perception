-- owner: WP-C.9
-- Clinician workspace foundation (MIG-2, docs/spec/modules/asd-clinician-and-interop/
-- technical.md; ADR-0008). Migration 0007 is intentionally unused and must stay unused
-- (MIG-1): sqlx would apply a later-added 0007 out of order on databases already at 0008.
--
-- Encryption (KEY-1..7): free text and the MRN are encrypted in the application
-- (AES-256-GCM, `asd-backend/src/crypto/`) before they reach SQL. Each encrypted column
-- `<name>_ct bytea` has a sibling `<name>_key_id uuid` naming the org data key (DEK) in
-- `org_data_keys`; the AAD binds a ciphertext to "{table}:{column}:{org_id}:{row_id}" with
-- column = the `_ct` column name. pgcrypto is not used (ADR-0008 D5). The `_key_id`
-- columns deliberately have no foreign key: deleting an org's `org_data_keys` rows is the
-- documented crypto-shredding action (KEY-5) and must not be blocked or cascade.
--
-- Immutability (SIGN-7, NOTE-1, NOTE-3) is enforced by triggers, as for `audit_log`
-- (AUTH-5, 0002): they apply to every role, the schema owner included.

-- ---------------------------------------------------------------- keys (KEY-2, KEY-5)

-- One wrapped 256-bit DEK per key id; at most one `active` key per org. `wrapped_dek` is
-- AES-256-GCM under ASD_DATA_KEK (layout version || nonce || ciphertext || tag, AAD
-- "org_data_keys:wrapped_dek:{org_id}:{key_id}"); `kek_id` is the KEK fingerprint, so a
-- wrong KEK is diagnosed instead of failing as a decryption error.
CREATE TABLE org_data_keys (
    key_id      uuid        PRIMARY KEY,
    org_id      uuid        NOT NULL REFERENCES organizations (id),
    wrapped_dek bytea       NOT NULL,
    kek_id      text        NOT NULL,
    status      text        NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'retired')),
    created_at  timestamptz NOT NULL DEFAULT now(),
    retired_at  timestamptz
);
CREATE UNIQUE INDEX org_data_keys_one_active_idx ON org_data_keys (org_id) WHERE status = 'active';
CREATE INDEX org_data_keys_org_created_idx ON org_data_keys (org_id, created_at);

-- ---------------------------------------------------------------- staff profile (NOTE-6)

-- Staff data, in clear (G9). Signing requires display_name (signer-profile-incomplete).
ALTER TABLE users
    ADD COLUMN display_name text CHECK (char_length(display_name) BETWEEN 1 AND 120),
    ADD COLUMN professional_registration text
        CHECK (char_length(professional_registration) BETWEEN 1 AND 60);

-- ---------------------------------------------------------------- review notes (KEY-7)

-- From 0008 on the API writes only note_ct/note_key_id. `asd-backend admin
-- encrypt-review-notes` encrypts older rows and sets note = NULL; a later migration drops
-- `note` once every deployment has run it.
ALTER TABLE review_items
    ADD COLUMN note_ct bytea,
    ADD COLUMN note_key_id uuid,
    ADD CONSTRAINT review_items_note_ct_key CHECK ((note_ct IS NULL) = (note_key_id IS NULL));
-- The back-fill scans for what is left.
CREATE INDEX review_items_plain_note_idx ON review_items (id) WHERE note IS NOT NULL;

-- ---------------------------------------------------------------- patient reference (MRN)

-- MRN-3: at most one MRN per participant, and one participant per (org, authority, MRN).
-- mrn_bidx = HMAC-SHA256(org index key, normalize(authority) || 0x1F || normalize(mrn))
-- (KEY-4); the MRN itself is only in mrn_ct.
CREATE TABLE patient_refs (
    id                  uuid        PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id              uuid        NOT NULL REFERENCES organizations (id),
    participant_id      uuid        NOT NULL,
    assigning_authority text        NOT NULL CHECK (char_length(assigning_authority) BETWEEN 1 AND 40),
    mrn_ct              bytea       NOT NULL,
    mrn_key_id          uuid        NOT NULL,
    mrn_bidx            bytea       NOT NULL CHECK (octet_length(mrn_bidx) = 32),
    created_by          uuid        REFERENCES users (id),
    created_at          timestamptz NOT NULL DEFAULT now(),
    updated_by          uuid        REFERENCES users (id),
    updated_at          timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT patient_refs_participant_key UNIQUE (participant_id),
    CONSTRAINT patient_refs_org_authority_bidx_key UNIQUE (org_id, assigning_authority, mrn_bidx),
    CONSTRAINT patient_refs_participant_same_org_fk FOREIGN KEY (participant_id, org_id)
        REFERENCES participants (id, org_id)
);

-- ---------------------------------------------------------------- clinical review (SIGN)

-- Lets clinical rows reference (session, org) so a review can never point at another
-- org's session.
ALTER TABLE sessions ADD CONSTRAINT sessions_id_org_key UNIQUE (id, org_id);

-- SIGN-1: one review per session. row_version backs If-Match (SIGN-3).
CREATE TABLE clinical_reviews (
    id              uuid        PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id          uuid        NOT NULL REFERENCES organizations (id),
    session_id      text        NOT NULL,
    state           text        NOT NULL DEFAULT 'open'
                        CHECK (state IN ('open', 'in_progress', 'awaiting_cosign', 'final', 'amending')),
    assigned_to     uuid        REFERENCES users (id),
    cosign_required boolean     NOT NULL DEFAULT false,
    row_version     integer     NOT NULL DEFAULT 1 CHECK (row_version >= 1),
    created_by      uuid        REFERENCES users (id),
    created_at      timestamptz NOT NULL DEFAULT now(),
    updated_at      timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT clinical_reviews_session_key UNIQUE (session_id),
    CONSTRAINT clinical_reviews_session_same_org_fk FOREIGN KEY (session_id, org_id)
        REFERENCES sessions (id, org_id)
);
CREATE INDEX clinical_reviews_org_state_idx ON clinical_reviews (org_id, state);

-- Versions of the review content. A draft is edited in place; signing sets status,
-- content_hash and signed_at; an amendment is a new draft with version_no + 1 and the
-- earlier signed version becomes `superseded` when the amendment is signed (SIGN-8).
CREATE TABLE clinical_review_versions (
    id                     uuid        PRIMARY KEY DEFAULT gen_random_uuid(),
    review_id              uuid        NOT NULL REFERENCES clinical_reviews (id),
    org_id                 uuid        NOT NULL REFERENCES organizations (id),
    version_no             integer     NOT NULL CHECK (version_no >= 1),
    status                 text        NOT NULL DEFAULT 'draft'
                               CHECK (status IN ('draft', 'signed', 'superseded')),
    disposition            text        CHECK (disposition IN ('no_concern_noted', 'monitor',
                               'refer_for_evaluation', 'recapture_requested', 'not_interpretable')),
    impression_ct          bytea,
    impression_key_id      uuid,
    recommendations_ct     bytea,
    recommendations_key_id uuid,
    amend_reason_ct        bytea,
    amend_reason_key_id    uuid,
    content_hash           bytea       CHECK (octet_length(content_hash) = 32),
    created_by             uuid        REFERENCES users (id),
    created_at             timestamptz NOT NULL DEFAULT now(),
    updated_by             uuid        REFERENCES users (id),
    updated_at             timestamptz NOT NULL DEFAULT now(),
    signed_at              timestamptz,
    superseded_at          timestamptz,
    CONSTRAINT clinical_review_versions_no_key UNIQUE (review_id, version_no),
    CONSTRAINT clinical_review_versions_impression_key
        CHECK ((impression_ct IS NULL) = (impression_key_id IS NULL)),
    CONSTRAINT clinical_review_versions_recommendations_key
        CHECK ((recommendations_ct IS NULL) = (recommendations_key_id IS NULL)),
    CONSTRAINT clinical_review_versions_amend_reason_key
        CHECK ((amend_reason_ct IS NULL) = (amend_reason_key_id IS NULL)),
    -- A signed (or superseded) version has a disposition, a hash and a signing time.
    CONSTRAINT clinical_review_versions_signed_complete CHECK (
        status = 'draft'
        OR (disposition IS NOT NULL AND content_hash IS NOT NULL AND signed_at IS NOT NULL)
    )
);
CREATE INDEX clinical_review_versions_review_idx ON clinical_review_versions (review_id, version_no DESC);
-- At most one open draft per review.
CREATE UNIQUE INDEX clinical_review_versions_one_draft_idx ON clinical_review_versions (review_id)
    WHERE status = 'draft';

-- SIGN-7: drafts change freely (including draft -> signed); a signed version changes only
-- to `superseded` (status and superseded_at, nothing else); a superseded version never
-- changes. Only drafts may be deleted (an abandoned amendment).
CREATE FUNCTION clinical_review_versions_guard() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP = 'DELETE' THEN
        IF OLD.status <> 'draft' THEN
            RAISE EXCEPTION 'clinical_review_versions: a % version is immutable (DELETE refused)', OLD.status
                USING ERRCODE = 'insufficient_privilege';
        END IF;
        RETURN OLD;
    END IF;
    IF OLD.status = 'draft' THEN
        RETURN NEW;
    END IF;
    IF OLD.status = 'signed' AND NEW.status = 'superseded'
       AND (to_jsonb(NEW) - 'status' - 'superseded_at') = (to_jsonb(OLD) - 'status' - 'superseded_at') THEN
        RETURN NEW;
    END IF;
    RAISE EXCEPTION 'clinical_review_versions: a % version is immutable (only signed -> superseded)', OLD.status
        USING ERRCODE = 'insufficient_privilege';
END;
$$;

CREATE TRIGGER clinical_review_versions_immutable
    BEFORE UPDATE OR DELETE ON clinical_review_versions
    FOR EACH ROW EXECUTE FUNCTION clinical_review_versions_guard();

-- SIGN-5/6: append-only signatures. The signer's display name and registration are a
-- snapshot taken at signing (signature manifestation). A cosign names the primary signer;
-- the CHECK refuses a self-cosign and the trigger below makes the named primary signer the
-- version's actual signer, so the CHECK cannot be sidestepped.
CREATE TABLE clinical_signatures (
    id                  uuid        PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id              uuid        NOT NULL REFERENCES organizations (id),
    review_id           uuid        NOT NULL REFERENCES clinical_reviews (id),
    version_id          uuid        NOT NULL REFERENCES clinical_review_versions (id),
    signer_id           uuid        NOT NULL REFERENCES users (id),
    role                text        NOT NULL CHECK (role IN ('sign', 'cosign')),
    meaning             text        NOT NULL,
    content_hash        bytea       NOT NULL CHECK (octet_length(content_hash) = 32),
    signer_display_name text        NOT NULL CHECK (char_length(signer_display_name) BETWEEN 1 AND 120),
    signer_registration text        CHECK (char_length(signer_registration) BETWEEN 1 AND 60),
    primary_signer_id   uuid        REFERENCES users (id),
    signed_at           timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT clinical_signatures_one_per_role UNIQUE (version_id, role),
    CONSTRAINT clinical_signatures_cosigner_differs CHECK (
        (role = 'sign' AND primary_signer_id IS NULL)
        OR (role = 'cosign' AND primary_signer_id IS NOT NULL AND primary_signer_id <> signer_id)
    )
);
CREATE INDEX clinical_signatures_review_idx ON clinical_signatures (review_id, signed_at);

CREATE FUNCTION clinical_signatures_cosign_check() RETURNS trigger
LANGUAGE plpgsql AS $$
DECLARE
    primary_signer uuid;
BEGIN
    IF NEW.role <> 'cosign' THEN
        RETURN NEW;
    END IF;
    SELECT signer_id INTO primary_signer
        FROM clinical_signatures WHERE version_id = NEW.version_id AND role = 'sign';
    IF primary_signer IS NULL THEN
        RAISE EXCEPTION 'clinical_signatures: cosign of a version with no primary signature'
            USING ERRCODE = 'check_violation';
    END IF;
    IF primary_signer = NEW.signer_id
       OR (NEW.primary_signer_id IS NOT NULL AND NEW.primary_signer_id <> primary_signer) THEN
        RAISE EXCEPTION 'clinical_signatures: cosigner must differ from the primary signer'
            USING ERRCODE = 'check_violation';
    END IF;
    NEW.primary_signer_id := primary_signer;
    RETURN NEW;
END;
$$;

CREATE TRIGGER clinical_signatures_cosign
    BEFORE INSERT ON clinical_signatures
    FOR EACH ROW EXECUTE FUNCTION clinical_signatures_cosign_check();

-- ---------------------------------------------------------------- notes (NOTE-1, NOTE-2)

-- Append-only; a correction is a new row with corrects_id (kind = 'correction').
CREATE TABLE clinical_notes (
    id             uuid        PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id         uuid        NOT NULL REFERENCES organizations (id),
    participant_id uuid        NOT NULL,
    session_id     text,
    kind           text        NOT NULL DEFAULT 'general' CHECK (kind IN ('general', 'correction')),
    body_ct        bytea       NOT NULL,
    body_key_id    uuid        NOT NULL,
    corrects_id    uuid        REFERENCES clinical_notes (id),
    author_id      uuid        NOT NULL REFERENCES users (id),
    created_at     timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT clinical_notes_correction_kind CHECK ((kind = 'correction') = (corrects_id IS NOT NULL)),
    CONSTRAINT clinical_notes_participant_same_org_fk FOREIGN KEY (participant_id, org_id)
        REFERENCES participants (id, org_id),
    CONSTRAINT clinical_notes_session_same_org_fk FOREIGN KEY (session_id, org_id)
        REFERENCES sessions (id, org_id)
);
CREATE INDEX clinical_notes_participant_created_idx ON clinical_notes (participant_id, created_at);
CREATE INDEX clinical_notes_corrects_idx ON clinical_notes (corrects_id) WHERE corrects_id IS NOT NULL;

-- ---------------------------------------------------------------- letters (NOTE-3)

-- Drafts are edited in place (If-Match on row_version); finalize stores the letter hash and
-- the finalizer's name/registration snapshot, after which the row never changes.
CREATE TABLE referral_letters (
    id                  uuid        PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id              uuid        NOT NULL REFERENCES organizations (id),
    participant_id      uuid        NOT NULL,
    kind                text        NOT NULL CHECK (kind IN ('referral', 'summary')),
    status              text        NOT NULL DEFAULT 'draft' CHECK (status IN ('draft', 'final')),
    session_ids         text[]      NOT NULL DEFAULT '{}',
    recipient_ct        bytea,
    recipient_key_id    uuid,
    body_ct             bytea,
    body_key_id         uuid,
    row_version         integer     NOT NULL DEFAULT 1 CHECK (row_version >= 1),
    content_hash        bytea       CHECK (octet_length(content_hash) = 32),
    author_id           uuid        NOT NULL REFERENCES users (id),
    finalized_by        uuid        REFERENCES users (id),
    finalized_at        timestamptz,
    signer_display_name text        CHECK (char_length(signer_display_name) BETWEEN 1 AND 120),
    signer_registration text        CHECK (char_length(signer_registration) BETWEEN 1 AND 60),
    created_at          timestamptz NOT NULL DEFAULT now(),
    updated_at          timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT referral_letters_recipient_key CHECK ((recipient_ct IS NULL) = (recipient_key_id IS NULL)),
    CONSTRAINT referral_letters_body_key CHECK ((body_ct IS NULL) = (body_key_id IS NULL)),
    CONSTRAINT referral_letters_final_complete CHECK (
        status = 'draft'
        OR (content_hash IS NOT NULL AND finalized_by IS NOT NULL AND finalized_at IS NOT NULL
            AND signer_display_name IS NOT NULL)
    ),
    CONSTRAINT referral_letters_participant_same_org_fk FOREIGN KEY (participant_id, org_id)
        REFERENCES participants (id, org_id)
);
CREATE INDEX referral_letters_participant_idx ON referral_letters (participant_id, created_at DESC);

CREATE FUNCTION referral_letters_guard() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    IF OLD.status = 'final' THEN
        RAISE EXCEPTION 'referral_letters: a finalized letter is immutable (% refused)', TG_OP
            USING ERRCODE = 'insufficient_privilege';
    END IF;
    IF TG_OP = 'DELETE' THEN
        RETURN OLD;
    END IF;
    RETURN NEW;
END;
$$;

CREATE TRIGGER referral_letters_immutable
    BEFORE UPDATE OR DELETE ON referral_letters
    FOR EACH ROW EXECUTE FUNCTION referral_letters_guard();

-- ---------------------------------------------------------------- append-only guards

CREATE FUNCTION clinical_append_only() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION '% is append-only (% refused)', TG_TABLE_NAME, TG_OP
        USING ERRCODE = 'insufficient_privilege';
END;
$$;

CREATE TRIGGER clinical_signatures_no_update_delete
    BEFORE UPDATE OR DELETE ON clinical_signatures
    FOR EACH ROW EXECUTE FUNCTION clinical_append_only();
CREATE TRIGGER clinical_signatures_no_truncate
    BEFORE TRUNCATE ON clinical_signatures
    FOR EACH STATEMENT EXECUTE FUNCTION clinical_append_only();

CREATE TRIGGER clinical_notes_no_update_delete
    BEFORE UPDATE OR DELETE ON clinical_notes
    FOR EACH ROW EXECUTE FUNCTION clinical_append_only();
CREATE TRIGGER clinical_notes_no_truncate
    BEFORE TRUNCATE ON clinical_notes
    FOR EACH STATEMENT EXECUTE FUNCTION clinical_append_only();

-- Signed versions and finalized letters cannot be emptied wholesale either.
CREATE TRIGGER clinical_review_versions_no_truncate
    BEFORE TRUNCATE ON clinical_review_versions
    FOR EACH STATEMENT EXECUTE FUNCTION clinical_append_only();
CREATE TRIGGER referral_letters_no_truncate
    BEFORE TRUNCATE ON referral_letters
    FOR EACH STATEMENT EXECUTE FUNCTION clinical_append_only();

-- ---------------------------------------------------------------- application role grants

-- The default privileges from 0002 give asd_app DML on these tables; take back what the
-- triggers refuse anyway, so a misbehaving API gets a permission error first.
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'asd_app') THEN
        RETURN;
    END IF;
    REVOKE UPDATE, DELETE, TRUNCATE ON clinical_signatures FROM asd_app;
    REVOKE UPDATE, DELETE, TRUNCATE ON clinical_notes FROM asd_app;
    REVOKE TRUNCATE ON clinical_review_versions, referral_letters FROM asd_app;
END;
$$;
