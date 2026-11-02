-- owner: WP-B.14 (ADR-0009 decision 5, KEY-11, KEY-12): drop the plaintext copies.
--
-- `users.totp_secret` (0001; sealed into `totp_secret_ct` since 0016) and
-- `review_items.note` (0003; sealed into `note_ct` since 0008) are removed. The service
-- stopped reading and writing both in the same release.
--
-- Nothing is ever NULLed here: a leftover plaintext seed would silently un-enrol its user
-- and a leftover note would be lost. The guard fails the migration instead (the whole
-- migration is one transaction, so nothing is dropped and `asd-backend migrate` can run
-- again once the data is converted):
--   * TOTP seeds: `asd-backend migrate` seals them *before* applying pending migrations
--     (KEY-12), so this fires only without ASD_DATA_KEK or when the migrations are applied
--     by another tool;
--   * review notes: `asd-backend admin encrypt-review-notes` was removed together with this
--     migration, so it must be run with the previous release (any build with 0008..0021)
--     before upgrading (rust/README.md, "Upgrade notes").

DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM information_schema.columns
               WHERE table_schema = current_schema() AND table_name = 'users'
                 AND column_name = 'totp_secret') THEN
        IF EXISTS (SELECT 1 FROM users WHERE totp_secret IS NOT NULL) THEN
            RAISE EXCEPTION 'plaintext TOTP seeds remain: run "asd-backend migrate" with ASD_DATA_KEK set';
        END IF;
    END IF;
    IF EXISTS (SELECT 1 FROM information_schema.columns
               WHERE table_schema = current_schema() AND table_name = 'review_items'
                 AND column_name = 'note') THEN
        IF EXISTS (SELECT 1 FROM review_items WHERE note IS NOT NULL) THEN
            RAISE EXCEPTION 'plaintext review notes remain: run "asd-backend admin encrypt-review-notes" with the previous release (before upgrading to a build with migration 0022)';
        END IF;
    END IF;
END
$$;

DROP INDEX IF EXISTS review_items_plain_note_idx;
ALTER TABLE users DROP CONSTRAINT IF EXISTS users_totp_secret_one_form;
ALTER TABLE users DROP COLUMN IF EXISTS totp_secret;
ALTER TABLE review_items DROP COLUMN IF EXISTS note;
