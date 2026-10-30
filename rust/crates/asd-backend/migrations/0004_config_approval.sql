-- owner: WP-B.4
-- Config document approval (CFG-1, four-eyes rule). The lifecycle
-- `draft -> candidate -> published -> retired` keeps its CHECK from 0001; approving a
-- draft makes it a `candidate`, and the approver and publisher are recorded so the
-- four-eyes rule (nobody publishes their own document; a system-seeded document needs
-- two different people) can be checked from the row itself. audit_log keeps the history.

ALTER TABLE config_documents ADD COLUMN approved_by  uuid REFERENCES users (id);
ALTER TABLE config_documents ADD COLUMN approved_at  timestamptz;
ALTER TABLE config_documents ADD COLUMN published_by uuid REFERENCES users (id);

-- At most one published document per kind: publishing retires the previous one in the
-- same transaction, and this index turns a concurrent double publish into an error
-- instead of two "current" documents.
CREATE UNIQUE INDEX config_documents_one_published_idx
    ON config_documents (kind) WHERE status = 'published';

-- Console lists (`GET /v1/config/{kind}`) newest first.
CREATE INDEX config_documents_kind_created_idx
    ON config_documents (kind, created_at DESC, id DESC);
