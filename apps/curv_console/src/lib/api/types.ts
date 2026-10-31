/**
 * Contract types for the ASD backend `/v1` user plane. Source of truth: the Rust `asd-contract::api_v1` types,
 * exported as docs/spec/schemas/asd/api-v1.json. `src/lib/api/contract.test.ts` checks these schemas against
 * that file (examples, enums, required fields), so contract drift fails CI. Every live response is also parsed here.
 */
import { z } from "zod";

/** `SessionAttentionFlag`: the 3 device values plus backend-derived `unreliable` (CON-15). */
export const ATTENTION_FLAGS = ["green", "yellow", "red", "unreliable"] as const;
export const attentionFlagSchema = z.enum(ATTENTION_FLAGS);
export type AttentionFlag = z.infer<typeof attentionFlagSchema>;

/** `ExpectationStatus` of a metric row / clip. */
export const CLIP_STATUSES = ["onTarget", "monitor", "atypical"] as const;
export const clipStatusSchema = z.enum(CLIP_STATUSES);
export type ClipStatus = z.infer<typeof clipStatusSchema>;

export const CAPTURE_SOURCES = ["truedepth", "simulated", "synthetic"] as const;
export const captureSourceSchema = z.enum(CAPTURE_SOURCES);
export type CaptureSource = z.infer<typeof captureSourceSchema>;

export const DIRECTIONS = ["higherBetter", "lowerBetter", "band"] as const;
export const directionSchema = z.enum(DIRECTIONS);

export const ORG_KINDS = ["home", "clinic", "research"] as const;

/** `GazeQualityVerdict`. The wire field is a free string; these are the values the backend stores. */
export const QUALITY_VERDICTS = ["optimal", "acceptable", "degraded", "rejected"] as const;

export const ROLES = ["owner", "admin", "clinician", "researcher", "reviewer", "viewer", "superadmin"] as const;
export const roleSchema = z.enum(ROLES);
export type Role = z.infer<typeof roleSchema>;

/** `UserStatus`: `invited` accounts exist but cannot sign in yet. */
export const USER_STATUSES = ["active", "disabled", "invited"] as const;
export const userStatusSchema = z.enum(USER_STATUSES);
export type UserStatus = z.infer<typeof userStatusSchema>;

const num = z.number().nullable().optional();
const str = z.string().nullable().optional();

// ---- auth -----------------------------------------------------------------

export const userSchema = z.object({
  id: z.string(),
  email: z.string(),
  role: roleSchema,
  /** null only for the global superadmin, who belongs to no org. */
  org_id: z.string().nullable(),
});
export type User = z.infer<typeof userSchema>;

/** POST /auth/login and POST /auth/refresh response. */
export const tokenPairSchema = z.object({
  access_token: z.string().min(1),
  refresh_token: z.string().min(1),
  /** Access-token lifetime in seconds. */
  expires_in: z.number().int().positive(),
  user: userSchema,
});
export type TokenPair = z.infer<typeof tokenPairSchema>;

// ---- org ------------------------------------------------------------------

export const orgSchema = z.object({
  id: z.string(),
  name: z.string(),
  kind: z.enum(ORG_KINDS),
  retention_months: z.number().int().nullable().optional(),
});
export type Org = z.infer<typeof orgSchema>;

// ---- metrics --------------------------------------------------------------

/** GET /orgs/{id}/metrics */
export const metricsSchema = z.object({
  sessions_7d: z.number().int(),
  sessions_30d: z.number().int(),
  flag_distribution: z.record(z.string(), z.number().int()),
  quality_distribution: z.record(z.string(), z.number().int()),
  review_queue_size: z.number().int(),
  devices_active: z.number().int(),
  bundle_adoption: z.array(z.object({ bundle_version: z.string(), devices: z.number().int() })),
});
export type Metrics = z.infer<typeof metricsSchema>;

// ---- sessions -------------------------------------------------------------

export const sessionSummarySchema = z.object({
  id: z.string(),
  /** `participants.id` (UUID, CON-20). */
  participant_id: z.string(),
  /** Pseudonymous token the device sent; the human-facing label (CON-20). */
  participant_token: z.string(),
  started_at: z.string(),
  duration_sec: z.number(),
  /** Catalog protocol id such as `express_30s` (CON-16); never an enum. */
  battery: z.string().nullable().optional(),
  capture_source: captureSourceSchema.nullable().optional(),
  quality_verdict: z.string(),
  attention_flag: attentionFlagSchema,
  composite_score: num,
  composite_confidence: num,
  bundle_version: str,
  device_id: str,
  reviewed: z.boolean().nullable().optional(),
});
export type SessionSummary = z.infer<typeof sessionSummarySchema>;

export const sessionDetailSchema = sessionSummarySchema.extend({
  org_id: z.string(),
  age_pack: str,
  lang: str,
  schema_version: str,
  valid_frame_ratio: num,
  calibrated_distance_cm: num,
  social_preference: num,
  eye_dwell: num,
  mouth_dwell: num,
  first_eye_latency_ms: num,
  affect: z.record(z.string(), z.unknown()).nullable().optional(),
  report_pdf_available: z.boolean().nullable().optional(),
});
export type SessionDetail = z.infer<typeof sessionDetailSchema>;

/** One scored expected-vs-recorded cell of a clip (CON-19). */
export const metricRowSchema = z.object({
  segment_id: z.string(),
  metric: z.string(),
  target_aoi: str,
  direction: directionSchema,
  expected: num,
  acceptable_low: num,
  acceptable_high: num,
  recorded: z.number(),
  /** `recorded - expected`, computed by the backend; null when no expected value. */
  deviation: num,
  status: clipStatusSchema,
  score: z.number(),
  imputed: z.boolean(),
  source: z.string(),
});
export type MetricRow = z.infer<typeof metricRowSchema>;

/** GET /sessions/{id}/clips item. Flat metric columns mirror the worst-status row; `rows` has them all. */
export const clipScoreSchema = z.object({
  clip_id: z.string(),
  order: z.number().int(),
  /** Worst row status; null for a clip with no scored rows. */
  status: clipStatusSchema.nullable().optional(),
  score: num,
  weight: num,
  dwell_social: num,
  dwell_geometric: num,
  dwell_eye: num,
  dwell_mouth: num,
  orienting_latency_ms: num,
  gaze_cue_accuracy: num,
  fixation_count: z.number().int().nullable().optional(),
  mean_fixation_ms: num,
  metric: str,
  expected: num,
  recorded: num,
  deviation: num,
  baseline_ref: str,
  rows: z.array(metricRowSchema).default([]),
});
export type ClipScore = z.infer<typeof clipScoreSchema>;

export const clipListSchema = z.object({ items: z.array(clipScoreSchema) });

export const sessionFiltersSchema = z.object({
  flag: attentionFlagSchema.optional(),
  quality: z.string().optional(),
  from: z.string().optional(),
  to: z.string().optional(),
  bundle_version: z.string().optional(),
  reviewed: z.boolean().optional(),
});
export type SessionFilters = z.infer<typeof sessionFiltersSchema>;

export interface SessionListQuery extends SessionFilters {
  cursor?: string;
  limit?: number;
}

/** Cursor-paginated list envelope (all list endpoints). */
export const sessionPageSchema = z.object({
  items: z.array(sessionSummarySchema),
  next_cursor: z.string().nullable().optional(),
  total: z.number().int().nullable().optional(),
});
export type SessionPage = z.infer<typeof sessionPageSchema>;

// ---- review queue ---------------------------------------------------------

/** `ReviewReason`: why the backend put a session in the queue (rules run at ingest, ING-3). */
export const REVIEW_REASONS = ["low_quality", "flag_red", "random_audit", "norm_candidate", "manual"] as const;
export const reviewReasonSchema = z.enum(REVIEW_REASONS);
export type ReviewReason = z.infer<typeof reviewReasonSchema>;

/** `ReviewStatus`. `done` and `dismissed` are terminal for the queue. */
export const REVIEW_STATUSES = ["open", "in_review", "done", "dismissed"] as const;
export const reviewStatusSchema = z.enum(REVIEW_STATUSES);
export type ReviewStatus = z.infer<typeof reviewStatusSchema>;

/** `LabelInput`: a human annotation attached by a reviewer. `value` is any JSON. */
export const labelInputSchema = z.object({
  key: z.string(),
  value: z.unknown().refine((v) => v !== undefined, "value is required"),
  clip_id: str,
});
export type LabelInput = z.infer<typeof labelInputSchema>;

/** GET /orgs/{id}/review item. */
export const reviewItemSchema = z.object({
  id: z.string(),
  org_id: z.string(),
  session_id: z.string(),
  reason: reviewReasonSchema,
  status: reviewStatusSchema,
  created_at: z.string(),
  assigned_to: str,
  /** Free JSON written by the console's triage action. */
  decision: z.unknown().optional(),
  decided_by: str,
  decided_at: str,
});
export type ReviewItem = z.infer<typeof reviewItemSchema>;

export const reviewPageSchema = z.object({
  items: z.array(reviewItemSchema),
  next_cursor: z.string().nullable().optional(),
  total: z.number().int().nullable().optional(),
});
export type ReviewPage = z.infer<typeof reviewPageSchema>;

/** PATCH /review/{id}: absent members are unchanged; labels are appended. */
export const updateReviewRequestSchema = z.object({
  status: reviewStatusSchema.nullable().optional(),
  assigned_to: str,
  decision: z.unknown().optional(),
  labels: z.array(labelInputSchema).optional(),
});
export type UpdateReviewRequest = z.infer<typeof updateReviewRequestSchema>;

export interface ReviewListQuery {
  status?: ReviewStatus;
  reason?: ReviewReason;
  cursor?: string;
  limit?: number;
}

// ---- devices --------------------------------------------------------------

/** GET /orgs/{id}/devices item. */
export const deviceSchema = z.object({
  id: z.string(),
  org_id: z.string(),
  platform: z.string(),
  app_version: z.string(),
  enrolled_at: z.string(),
  label: str,
  revoked_at: str,
  last_seen_at: str,
  config_bundle_version: str,
});
export type Device = z.infer<typeof deviceSchema>;

export const devicePageSchema = z.object({
  items: z.array(deviceSchema),
  next_cursor: z.string().nullable().optional(),
  total: z.number().int().nullable().optional(),
});
export type DevicePage = z.infer<typeof devicePageSchema>;

/** POST /orgs/{id}/devices/enrol-code body (all optional; backend defaults: 1 use, 7 days). */
export const enrolCodeRequestSchema = z.object({
  label: str,
  max_uses: z.number().int().min(1).nullable().optional(),
  expires_in_days: z.number().int().min(1).nullable().optional(),
});
export type EnrolCodeRequest = z.infer<typeof enrolCodeRequestSchema>;

/** The plain code is returned only by this response; the console never stores it. */
export const enrolCodeResponseSchema = z.object({
  id: z.string(),
  code: z.string().min(1),
  max_uses: z.number().int().min(0),
  expires_at: z.string(),
});
export type EnrolCodeResponse = z.infer<typeof enrolCodeResponseSchema>;

export interface PageQuery {
  cursor?: string;
  limit?: number;
}

// ---- client interface -----------------------------------------------------

export interface CurvApi {
  getOrg(orgId: string): Promise<Org>;
  getMetrics(orgId: string): Promise<Metrics>;
  listSessions(orgId: string, query?: SessionListQuery): Promise<SessionPage>;
  getSession(sessionId: string): Promise<SessionDetail>;
  getSessionClips(sessionId: string): Promise<ClipScore[]>;
  listReview(orgId: string, query?: ReviewListQuery): Promise<ReviewPage>;
  updateReview(reviewId: string, patch: UpdateReviewRequest): Promise<ReviewItem>;
  listDevices(orgId: string, query?: PageQuery): Promise<DevicePage>;
  revokeDevice(deviceId: string): Promise<Device>;
  createEnrolCode(orgId: string, request?: EnrolCodeRequest): Promise<EnrolCodeResponse>;
}

export interface AuthApi {
  login(email: string, password: string): Promise<TokenPair>;
  refresh(refreshToken: string): Promise<TokenPair>;
  logout(refreshToken: string): Promise<void>;
}

/** RFC 7807 problem+json body; `type` is `https://schemas.curv.org/asd/problems/<slug>`. */
export const problemSchema = z.object({
  type: z.string(),
  title: z.string(),
  status: z.number().int(),
  detail: z.string().nullable().optional(),
  instance: z.string().nullable().optional(),
  invalid_params: z.array(z.object({ name: z.string(), reason: z.string() })).optional(),
});
export type Problem = z.infer<typeof problemSchema>;

export const PROBLEM_BASE = "https://schemas.curv.org/asd/problems/";

/** `ProblemSlug`: the slugs the contract defines (routes may add their own, e.g. `email-taken`). */
export const PROBLEM_SLUGS = [
  "validation-failed",
  "unsupported-schema-version",
  "idempotency-key-reused",
  "enrol-code-exhausted",
  "capture-source-rejected",
  "unauthorized",
  "forbidden",
  "not-found",
  "rate-limited",
  "conflict",
  "participant-withdrawn",
  "internal",
] as const;
export type ProblemSlug = (typeof PROBLEM_SLUGS)[number];

export class ApiError extends Error {
  readonly status: number;
  readonly problem: Problem | null;
  /** Problem slug, e.g. `validation-failed`; null when the body was not problem+json. */
  get slug(): string | null {
    return this.problem?.type.startsWith(PROBLEM_BASE) ? this.problem.type.slice(PROBLEM_BASE.length) : null;
  }
  constructor(status: number, problem: Problem | null, message?: string) {
    super(message ?? problem?.detail ?? problem?.title ?? `API error ${status}`);
    this.name = "ApiError";
    this.status = status;
    this.problem = problem;
  }
}
