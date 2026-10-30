-- owner: WP-B.5
-- Job queue and norm-run scoping.
--   * `org_id` on jobs and norm_runs: the org whose sessions feed the run (NULL = every org,
--     superadmin only). Org-scoped users only ever see their own org's jobs and runs.
--   * claim bookkeeping: `locked_by` names the worker that holds a running job, so a
--     crashed worker's job can be returned to the queue after a visibility timeout.
--   * list indexes for `GET /v1/norm-runs` (newest first).
ALTER TABLE jobs ADD COLUMN org_id uuid REFERENCES organizations (id);
ALTER TABLE jobs ADD COLUMN locked_by text;
ALTER TABLE jobs ADD CONSTRAINT jobs_kind_nonempty CHECK (length(kind) > 0);
CREATE INDEX jobs_running_idx ON jobs (started_at) WHERE status = 'running';

ALTER TABLE norm_runs ADD COLUMN org_id uuid REFERENCES organizations (id);
CREATE INDEX norm_runs_created_idx ON norm_runs (created_at DESC, id DESC);
CREATE UNIQUE INDEX norm_runs_job_key ON norm_runs (job_id) WHERE job_id IS NOT NULL;
