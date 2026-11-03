/**
 * Fixture-backed erasure, retention-sweep and dataset-export API for mock mode (WP-B.9). Synthetic data only.
 * It imitates the backend flow the UI shows (pending -> completed or held, queued -> running -> succeeded) so
 * the pages can be tried in the demo; the real execution lives in the Rust backend jobs.
 */
import { ApiError, PROBLEM_BASE } from "../types";
import type { AdminApi } from "../admin";
import type { ExportsApi } from "../exports";
import type { ErasureRequest, JobView, PrivacyApi, RetentionSweepJob, RetentionSweepResult } from "../privacy";

export interface MockPrivacyContext {
  orgId: string;
  now: number;
  ensureOrg: (orgId: string) => void;
  page: <T>(
    rows: T[],
    key: (row: T) => [string, string],
    cursor: string | undefined,
    limit?: number,
  ) => { slice: T[]; next_cursor: string | null; total: number };
  /** Shared with the admin mock so withdrawn participants show as withdrawn in the participants list. */
  withdrawn: Map<string, string>;
  listParticipants: AdminApi["listParticipants"];
  createExport: ExportsApi["createExport"];
}

function problem(status: number, slug: string, title: string, detail?: string, params?: { name: string; reason: string }[]) {
  return new ApiError(status, {
    type: `${PROBLEM_BASE}${slug}`,
    title,
    status,
    detail,
    ...(params ? { invalid_params: params } : {}),
  });
}

const uuid = (kind: number, n: number) => `b9${kind}0c0de-0000-4000-8000-${String(n).padStart(12, "0")}`;
const DAY = 86_400_000;
const NO_OUTCOME = {
  sessions_deleted: 0, sessions_held: 0, care_events_deleted: 0, session_blobs_deleted: 0,
  export_artifacts_deleted: 0, clinical_drafts_deleted: 0, patient_ref_deleted: false,
};

interface MockJob {
  view: JobView;
  dryRun: boolean;
  polls: number;
}

export function createMockPrivacy(ctx: MockPrivacyContext): PrivacyApi {
  const iso = (offset: number) => new Date(ctx.now + offset).toISOString();
  let seq = 0;

  const seed = (n: number, over: Partial<ErasureRequest>): ErasureRequest => ({
    id: uuid(1, n), org_id: ctx.orgId, participant_id: uuid(2, n), participant_token: `P-SEED-${100 + n}`,
    status: "completed", scope: "all", requested_by_type: "user", requested_at: iso(-n * DAY),
    executed_at: iso(-n * DAY + 60_000), completed_at: iso(-n * DAY + 61_000), hold_reason: null,
    outcome: { ...NO_OUTCOME, sessions_deleted: 4, care_events_deleted: 2, session_blobs_deleted: 9, export_artifacts_deleted: 1, patient_ref_deleted: true },
    duplicate: false, ...over,
  });
  const requests: ErasureRequest[] = [
    seed(1, { status: "pending", executed_at: null, completed_at: null, outcome: null }),
    seed(2, {
      status: "held", hold_reason: "clinical_record", requested_by_type: "device",
      outcome: { ...NO_OUTCOME, sessions_deleted: 2, sessions_held: 3, session_blobs_deleted: 5, clinical_drafts_deleted: 1 },
    }),
    seed(3, {}),
  ];
  /** Requests filed in this process: they complete when the list is read a second time. */
  const live = new Set<string>();
  const holdParticipants = new Set<string>();

  const jobs = new Map<string, MockJob>();
  /** Sessions "past retention": a real sweep purges them, so a later dry run counts zero. */
  let purgeable = 17;

  function advance(): void {
    for (const r of requests) {
      if (r.status !== "pending" || !live.has(r.id)) continue;
      live.delete(r.id);
      // A fixed rule standing in for "has signed clinical records": the participants with an MRN mapping.
      const held = holdParticipants.has(r.participant_id);
      r.status = held ? "held" : "completed";
      r.hold_reason = held ? "clinical_record" : null;
      r.executed_at = iso(60_000 + requests.length);
      r.completed_at = held ? null : r.executed_at;
      r.outcome = { ...NO_OUTCOME, sessions_deleted: held ? 1 : 2, sessions_held: held ? 2 : 0, session_blobs_deleted: 4, patient_ref_deleted: !held };
    }
  }

  const sorted = () => [...requests].sort((a, b) => b.requested_at.localeCompare(a.requested_at) || b.id.localeCompare(a.id));
  const clone = <T,>(v: T): T => structuredClone(v);

  return {
    async listErasureRequests(org, query) {
      ctx.ensureOrg(org);
      const first = !query?.cursor;
      const snapshot = clone(sorted());
      if (first) advance();
      const { slice, next_cursor } = ctx.page(snapshot, (r) => [r.requested_at, r.id], query?.cursor, query?.limit);
      return { items: slice, next_cursor };
    },
    async requestParticipantErasure(org, participantId) {
      ctx.ensureOrg(org);
      const all = (await ctx.listParticipants(org, { limit: 100 })).items;
      const p = all.find((x) => x.id === participantId);
      if (!p) throw problem(404, "not-found", "Not found", "participant not found");
      const open = requests.find((r) => r.participant_id === participantId && r.status === "pending");
      if (open) return { ...clone(open), duplicate: true };
      seq += 1;
      const created: ErasureRequest = {
        id: uuid(1, 100 + seq), org_id: org, participant_id: p.id, participant_token: p.participant_token,
        status: "pending", scope: "all", requested_by_type: "user", requested_at: iso(seq * 1000),
        executed_at: null, completed_at: null, hold_reason: null, outcome: null, duplicate: false,
      };
      requests.push(created);
      live.add(created.id);
      if (p.has_patient_ref) holdParticipants.add(p.id);
      ctx.withdrawn.set(p.id, created.requested_at);
      return clone(created);
    },
    async startRetentionSweep(org, req) {
      ctx.ensureOrg(org);
      seq += 1;
      const job: RetentionSweepJob = {
        job_id: uuid(3, 100 + seq), org_id: org, dry_run: req.dry_run, status: "queued", created_at: iso(seq * 1000),
      };
      jobs.set(job.job_id, {
        dryRun: req.dry_run, polls: 0,
        view: { id: job.job_id, kind: "retention_sweep", status: "queued", result: null, error: null, created_at: job.created_at, finished_at: null },
      });
      return job;
    },
    async getJob(jobId) {
      const j = jobs.get(jobId);
      if (!j) throw problem(404, "not-found", "Not found", "job not found");
      j.polls += 1;
      if (j.polls === 1) j.view.status = "running";
      else if (j.view.status !== "succeeded") {
        const result: RetentionSweepResult = {
          dry_run: j.dryRun,
          orgs: [{ org_id: ctx.orgId, retention_months: 24, cutoff: iso(-730 * DAY), frames_purged: purgeable, research_frames_purged: 0 }],
        };
        if (!j.dryRun) purgeable = 0;
        j.view = { ...j.view, status: "succeeded", result, finished_at: iso(5_000) };
      }
      return clone(j.view);
    },
    async createDatasetExport(req) {
      const reason = req.reason.trim();
      if (reason.length < 10 || reason.length > 500) {
        throw problem(400, "validation-failed", "Validation failed", undefined, [{ name: "reason", reason: "must be 10..500 characters" }]);
      }
      return ctx.createExport(ctx.orgId, {
        format: req.format === "jsonl" ? "dataset_jsonl" : "dataset_parquet",
        mode: "deidentified",
        purpose: "research",
        reason,
        filters: req.filters,
        include_labels: req.include_labels ?? true,
      });
    },
  };
}
