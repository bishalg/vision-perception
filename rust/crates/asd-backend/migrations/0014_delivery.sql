-- owner: WP-I.7
-- Delivery channels (INTG-1..5, DLV-1..6; docs/spec/modules/asd-clinician-and-interop/
-- technical.md, ADR-0008). 0009 created `integration_destinations` and `export_deliveries`;
-- this adds what the channels need beyond it. Nothing here holds a secret in clear text.

-- INTG-5: the OpenSSH public key of the server-generated Ed25519 key of an SFTP
-- destination (the private seed is `secret_ct`, encrypted, KEY-1). Public by design: hospital
-- IT installs it in `authorized_keys`.
ALTER TABLE integration_destinations
    ADD COLUMN ssh_public_key text CHECK (ssh_public_key ~ '^ssh-ed25519 [A-Za-z0-9+/=]+$');

-- INTG-4: the short error class of the last connection test (never a remote body).
ALTER TABLE integration_destinations
    ADD COLUMN last_test_error text CHECK (last_test_error ~ '^[a-z0-9-]{1,64}$');

-- DLV-2: which webhook event a delivery announces. Manual exports announce `export.ready`;
-- auto-deliveries enqueued on final sign (DLV-6) announce `review.finalized`.
ALTER TABLE export_deliveries
    ADD COLUMN event text NOT NULL DEFAULT 'export.ready'
        CHECK (event IN ('export.ready', 'review.finalized'));

-- DLV-5: the error class is an enum spelling, never free remote text.
ALTER TABLE export_deliveries
    ADD CONSTRAINT export_deliveries_error_class_shape
        CHECK (last_error_class ~ '^[a-z0-9-]{1,64}$');

-- DLV-6: an auto-export names the review whose final signature produced it (one export per
-- finalisation, so amendments deliver again).
ALTER TABLE exports ADD COLUMN review_id uuid REFERENCES clinical_reviews (id);
CREATE INDEX exports_review_idx ON exports (review_id) WHERE review_id IS NOT NULL;

-- Deliveries of one export, for the export view and the "enqueue when ready" step.
CREATE INDEX export_deliveries_export_idx ON export_deliveries (export_id);
