-- owner: console gaps (I.8)
-- Deleting a delivery destination archives it: `export_deliveries` keeps a foreign key to the
-- row (the delivery log is evidence), so the destination is hidden, never removed.
-- `DELETE /v1/integrations/{id}` sets `archived_at` and `status = 'disabled'`; reads, lists
-- and the export destination check skip archived rows.
ALTER TABLE integration_destinations ADD COLUMN archived_at timestamptz;
