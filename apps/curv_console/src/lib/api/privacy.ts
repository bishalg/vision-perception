/**
 * Erasure, retention sweep and dataset export (WP-B.9): zod schemas for the DTOs of
 * `docs/spec/schemas/asd/api-v1.json`, the `PrivacyApi` interface and its live HTTP implementation. The console
 * only files requests and shows what the backend returns; it deletes and counts nothing itself.
 */
import { z } from "zod";
import type { LiveRequest } from "./clinical";
import { exportFiltersSchema, exportSchema, type Export } from "./exports";

const str = z.string().nullable().optional();
const count = z.number().int().min(0);

export const ERASURE_STATUSES = ["pending", "completed", "held"] as const;
export const erasureStatusSchema = z.enum(ERASURE_STATUSES);
export type ErasureStatus = z.infer<typeof erasureStatusSchema>;

/** `erasure_requests.hold_reason` when `status = held`. */
export const HOLD_CLINICAL_RECORD = "clinical_record";

export const DATASET_FORMATS = ["jsonl", "parquet"] as const;
export const datasetFormatSchema = z.enum(DATASET_FORMATS);
export type DatasetFormat = z.infer<typeof datasetFormatSchema>;

/** What one erasure removed. Counts only, never identifiers. */
export const erasureOutcomeSchema = z.object({
  sessions_deleted: count,
  sessions_held: count,
  care_events_deleted: count,
  session_blobs_deleted: count,
  export_artifacts_deleted: count,
  clinical_drafts_deleted: count,
  patient_ref_deleted: z.boolean(),
});
export type ErasureOutcome = z.infer<typeof erasureOutcomeSchema>;

export const erasureRequestSchema = z.object({
  id: z.string(),
  org_id: z.string(),
  participant_id: z.string(),
  participant_token: z.string(),
  status: erasureStatusSchema,
  scope: z.string(),
  /** `device | user | system`. */
  requested_by_type: z.string(),
  requested_at: z.string(),
  completed_at: str,
  executed_at: str,
  hold_reason: str,
  outcome: erasureOutcomeSchema.nullable().optional(),
  /** True when an open request already existed and is returned unchanged (HTTP 200 instead of 202). */
  duplicate: z.boolean().optional(),
});
export type ErasureRequest = z.infer<typeof erasureRequestSchema>;

export const erasureRequestPageSchema = z.object({
  items: z.array(erasureRequestSchema),
  next_cursor: z.string().nullable().optional(),
});
export type ErasureRequestPage = z.infer<typeof erasureRequestPageSchema>;

export const retentionSweepRequestSchema = z.object({
  /** Default `true` on the backend; the console always sends it explicitly. */
  dry_run: z.boolean().optional(),
});
export type RetentionSweepRequest = z.infer<typeof retentionSweepRequestSchema>;

export const retentionSweepJobSchema = z.object({
  job_id: z.string(),
  org_id: z.string(),
  dry_run: z.boolean(),
  /** `queued` when created. */
  status: z.string(),
  created_at: z.string(),
});
export type RetentionSweepJob = z.infer<typeof retentionSweepJobSchema>;

export const retentionOrgResultSchema = z.object({
  org_id: z.string(),
  retention_months: z.number().int(),
  cutoff: z.string(),
  frames_purged: count,
});
export type RetentionOrgResult = z.infer<typeof retentionOrgResultSchema>;

export const retentionSweepResultSchema = z.object({
  dry_run: z.boolean(),
  orgs: z.array(retentionOrgResultSchema),
});
export type RetentionSweepResult = z.infer<typeof retentionSweepResultSchema>;

/**
 * `GET /v1/jobs/{id}`. Not part of the published contract schema (the route returns a generic job view), so the
 * console reads only what it needs; `result` is parsed per job kind by the caller.
 */
export const JOB_STATUSES = ["queued", "running", "succeeded", "failed"] as const;
export const jobViewSchema = z.object({
  id: z.string(),
  kind: z.string(),
  status: z.string(),
  result: z.unknown().optional(),
  error: str,
  created_at: z.string().optional(),
  finished_at: str,
});
export type JobView = z.infer<typeof jobViewSchema>;

export function isJobFinished(job: Pick<JobView, "status">): boolean {
  return job.status === "succeeded" || job.status === "failed";
}

/** `POST /v1/jobs/export-dataset`: always de-identified, purpose research. */
export const datasetExportRequestSchema = z.object({
  format: datasetFormatSchema,
  /** 10..500 characters, shown in the audit log. */
  reason: z.string(),
  filters: exportFiltersSchema.optional(),
  /** Default `true` on the backend. */
  include_labels: z.boolean().optional(),
});
export type DatasetExportRequest = z.infer<typeof datasetExportRequestSchema>;

export interface PrivacyPageQuery {
  cursor?: string;
  /** 1..100 (backend cap). */
  limit?: number;
}

export interface PrivacyApi {
  /** Owners and admins (`ParticipantErase`); newest first. */
  listErasureRequests(orgId: string, query?: PrivacyPageQuery): Promise<ErasureRequestPage>;
  /** 202 with the new request, or 200 with the open one (`duplicate: true`). 404 for another org's participant. */
  requestParticipantErasure(orgId: string, participantId: string): Promise<ErasureRequest>;
  /** Queues a job; the counts arrive as the job's `result` (`getJob`). `dry_run` counts without deleting. */
  startRetentionSweep(orgId: string, request: { dry_run: boolean }): Promise<RetentionSweepJob>;
  getJob(jobId: string): Promise<JobView>;
  /** `ExportDeidentified`; superadmin gets 403. Same artifact flow as any export. */
  createDatasetExport(request: DatasetExportRequest): Promise<Export>;
}

export function createLivePrivacy(request: LiveRequest, buildQuery: (q: object) => string): PrivacyApi {
  const enc = encodeURIComponent;
  return {
    listErasureRequests: (orgId, q) =>
      request(`/orgs/${enc(orgId)}/erasure-requests${buildQuery(q ?? {})}`, erasureRequestPageSchema),
    requestParticipantErasure: (orgId, participantId) =>
      request(`/orgs/${enc(orgId)}/participants/${enc(participantId)}/erasure`, erasureRequestSchema, { method: "POST" }),
    startRetentionSweep: (orgId, body) =>
      request(`/orgs/${enc(orgId)}/retention-sweep`, retentionSweepJobSchema, { method: "POST", body }),
    getJob: (jobId) => request(`/jobs/${enc(jobId)}`, jobViewSchema),
    createDatasetExport: (body) => request(`/jobs/export-dataset`, exportSchema, { method: "POST", body }),
  };
}
