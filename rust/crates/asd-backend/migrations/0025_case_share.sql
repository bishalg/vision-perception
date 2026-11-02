-- owner: WP-I.13
-- Share a case (SHR-7, SHR-8; docs/spec/modules/asd-emr-lite-and-sharing/technical.md,
-- ADR-0011 D5): case-package exports and passcode share links. 0024 belongs to WP-B.19,
-- which runs in parallel (sqlx applies a lower number added later as long as it was never
-- applied).

-- ---------------------------------------------------------------- export formats

-- `case_package` (this package) and `audit_csv` (WP-B.23, which therefore needs no
-- migration of its own).
ALTER TABLE exports DROP CONSTRAINT exports_format_check;
ALTER TABLE exports ADD CONSTRAINT exports_format_check
    CHECK (format IN ('csv', 'csv_excel', 'csv_sheets', 'xlsx', 'pdf_summary', 'fhir_bundle',
                      'fhir_ndjson', 'hl7v2', 'dataset_jsonl', 'dataset_parquet',
                      'case_package', 'audit_csv'));

-- ---------------------------------------------------------------- case packages (SHR-7)

-- `case_participant_id`: the one participant of the package. No foreign key: like
-- `export_items`, the accounting of disclosures outlives erasure (EXJ-8).
-- `case_recipient_ct`: the recipient text, encrypted (KEY-1, AAD
-- "exports:case_recipient_ct:{org}:{export id}"). `case_consent_reference` is an
-- identifier such as a form number and stays in clear.
-- `case_include_attachments`: stored now, honoured by WP-I.13b.
ALTER TABLE exports
    ADD COLUMN case_participant_id      uuid,
    ADD COLUMN case_recipient_ct        bytea,
    ADD COLUMN case_recipient_key_id    uuid,
    ADD COLUMN case_consent_reference   text CHECK (char_length(case_consent_reference) BETWEEN 1 AND 200),
    ADD COLUMN case_consent_recorded_at timestamptz,
    ADD COLUMN case_include_attachments boolean NOT NULL DEFAULT false;
ALTER TABLE exports
    ADD CONSTRAINT exports_case_recipient_key
        CHECK ((case_recipient_ct IS NULL) = (case_recipient_key_id IS NULL)),
    -- A case package names its participant and recipient; no other format does.
    ADD CONSTRAINT exports_case_package_shape
        CHECK ((format = 'case_package') = (case_participant_id IS NOT NULL)
               AND (format = 'case_package') = (case_recipient_ct IS NOT NULL)),
    -- SHR-7: an identified package records the consent it relies on.
    ADD CONSTRAINT exports_case_identified_consent
        CHECK (format <> 'case_package' OR mode = 'deidentified'
               OR (case_consent_reference IS NOT NULL AND case_consent_recorded_at IS NOT NULL)),
    ADD CONSTRAINT exports_case_attachments_identified
        CHECK (NOT case_include_attachments OR (format = 'case_package' AND mode = 'identified'));
-- Erasure finds case packages of a participant even when no session was included
-- (WP-I.13b, SHR-11).
CREATE INDEX exports_case_participant_idx ON exports (case_participant_id)
    WHERE case_participant_id IS NOT NULL;

-- ---------------------------------------------------------------- share links (SHR-8)

-- The passcode is never stored: only its Argon2id PHC string (AUTH-1 parameters). A link
-- never outlives its artifact (`expires_at` is capped at the export's expiry when created)
-- nor seven days. `downloads` only grows, by the guarded `UPDATE ... WHERE downloads <
-- max_downloads` of the download route. Expired or revoked links delete nothing; the
-- artifact's own expiry (EXJ-4) governs.
CREATE TABLE share_links (
    id             uuid        PRIMARY KEY,
    org_id         uuid        NOT NULL REFERENCES organizations (id),
    export_id      uuid        NOT NULL,
    passcode_hash  text        NOT NULL CHECK (passcode_hash LIKE '$argon2id$%'),
    created_by     uuid        REFERENCES users (id),
    created_at     timestamptz NOT NULL DEFAULT now(),
    expires_at     timestamptz NOT NULL,
    max_downloads  integer     NOT NULL CHECK (max_downloads BETWEEN 1 AND 5),
    downloads      integer     NOT NULL DEFAULT 0 CHECK (downloads >= 0),
    opens          integer     NOT NULL DEFAULT 0 CHECK (opens >= 0),
    revoked_at     timestamptz,
    revoked_by     uuid        REFERENCES users (id),
    last_opened_at timestamptz,
    last_opened_ip inet,
    CONSTRAINT share_links_expiry
        CHECK (expires_at > created_at AND expires_at <= created_at + interval '7 days'),
    CONSTRAINT share_links_download_limit CHECK (downloads <= max_downloads),
    CONSTRAINT share_links_export_same_org_fk FOREIGN KEY (export_id, org_id)
        REFERENCES exports (id, org_id)
);
CREATE INDEX share_links_export_idx ON share_links (export_id, created_at DESC);

-- The default privileges of 0002 already give `asd_app` DML on new tables; granted
-- explicitly so the grant does not depend on who ran the migration (trap 17).
DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'asd_app') THEN
        GRANT SELECT, INSERT, UPDATE, DELETE ON share_links TO asd_app;
    END IF;
END
$$;
