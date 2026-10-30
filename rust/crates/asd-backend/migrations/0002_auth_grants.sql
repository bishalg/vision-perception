-- owner: WP-B.2
-- AUTH-5: `audit_log` is append-only; plus the application role's grants and the
-- enrol-code label (CON-11, `EnrolCodeRequest.label` is copied to the devices it enrols).
--
-- Roles: migrations run as the schema owner (`asd-backend migrate`). The API should log
-- in as a role that is a member of `asd_app`, e.g.
--   CREATE ROLE asd_api LOGIN PASSWORD '...' IN ROLE asd_app;
-- `asd_app` gets DML on every table except UPDATE/DELETE/TRUNCATE on `audit_log`. The
-- trigger below also refuses those statements for every other role, the owner included,
-- so the guarantee holds even where the API still connects as the owner (compose today).
-- A role without CREATEROLE skips the role and grants with a NOTICE; the trigger always
-- applies.

ALTER TABLE enrol_codes ADD COLUMN label text CHECK (char_length(label) <= 200);

-- ---------------------------------------------------------------- append-only audit_log

CREATE FUNCTION audit_log_append_only() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'audit_log is append-only (% refused)', TG_OP
        USING ERRCODE = 'insufficient_privilege';
END;
$$;

CREATE TRIGGER audit_log_no_update_delete
    BEFORE UPDATE OR DELETE ON audit_log
    FOR EACH ROW EXECUTE FUNCTION audit_log_append_only();

CREATE TRIGGER audit_log_no_truncate
    BEFORE TRUNCATE ON audit_log
    FOR EACH STATEMENT EXECUTE FUNCTION audit_log_append_only();

REVOKE UPDATE, DELETE, TRUNCATE ON audit_log FROM PUBLIC;

-- ---------------------------------------------------------------- application role

-- Roles are cluster-wide; parallel migrations of different databases (tests) may race to
-- create it, hence the exception handler.
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'asd_app') THEN
        BEGIN
            CREATE ROLE asd_app NOLOGIN;
        EXCEPTION
            WHEN duplicate_object OR unique_violation THEN
                NULL;
            WHEN insufficient_privilege THEN
                RAISE NOTICE 'asd_app not created (no CREATEROLE); grants skipped';
        END;
    END IF;
END;
$$;

DO $$
DECLARE
    schema_name text := current_schema();
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'asd_app') THEN
        RETURN;
    END IF;
    EXECUTE format('GRANT USAGE ON SCHEMA %I TO asd_app', schema_name);
    EXECUTE format(
        'GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA %I TO asd_app', schema_name);
    EXECUTE format('GRANT USAGE, SELECT ON ALL SEQUENCES IN SCHEMA %I TO asd_app', schema_name);
    -- Tables created by later migrations (same owner) get the same DML grants.
    EXECUTE format(
        'ALTER DEFAULT PRIVILEGES IN SCHEMA %I GRANT SELECT, INSERT, UPDATE, DELETE ON TABLES TO asd_app',
        schema_name);
    EXECUTE format(
        'ALTER DEFAULT PRIVILEGES IN SCHEMA %I GRANT USAGE, SELECT ON SEQUENCES TO asd_app',
        schema_name);

    -- AUTH-5: append-only for the application.
    REVOKE UPDATE, DELETE, TRUNCATE ON audit_log FROM asd_app;
    -- Readiness reads the migration table; only `asd-backend migrate` writes it.
    IF to_regclass('_sqlx_migrations') IS NOT NULL THEN
        REVOKE INSERT, UPDATE, DELETE, TRUNCATE ON _sqlx_migrations FROM asd_app;
    END IF;
END;
$$;
