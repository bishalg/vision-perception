/**
 * Clinician workspace API (WP-C.13; UI-5): zod schemas for the clinical DTOs of `docs/spec/schemas/asd/api-v1.json`,
 * the `ClinicalApi` interface, and the live HTTP implementation. The console holds no hashing or scoring: the
 * `content_hash` shown in dialogs is whatever the API returned, and is echoed back unchanged on sign/cosign.
 */
import { z } from "zod";
import { ApiError, orgSchema, roleSchema, sessionSummarySchema, type Org } from "./types";

const str = z.string().nullable().optional();

export const DISPOSITIONS = [
  "no_concern_noted",
  "monitor",
  "refer_for_evaluation",
  "recapture_requested",
  "not_interpretable",
] as const;
export const dispositionSchema = z.enum(DISPOSITIONS);
export type Disposition = z.infer<typeof dispositionSchema>;

export const CLINICAL_STATES = ["open", "in_progress", "awaiting_cosign", "final", "amending"] as const;
export const clinicalStateSchema = z.enum(CLINICAL_STATES);
export type ClinicalState = z.infer<typeof clinicalStateSchema>;

export const REVIEW_VERSION_STATUSES = ["draft", "signed", "superseded"] as const;
export const reviewVersionStatusSchema = z.enum(REVIEW_VERSION_STATUSES);

export const SIGNATURE_ROLES = ["sign", "cosign"] as const;
export const signatureRoleSchema = z.enum(SIGNATURE_ROLES);

export const COSIGN_POLICIES = ["none", "red_flag", "always"] as const;
export const cosignPolicySchema = z.enum(COSIGN_POLICIES);
export type CosignPolicy = z.infer<typeof cosignPolicySchema>;

export const WORKLIST_SCOPES = ["all", "flagged"] as const;
export const worklistScopeSchema = z.enum(WORKLIST_SCOPES);
export type WorklistScope = z.infer<typeof worklistScopeSchema>;

export const PATIENT_REF_MODES = ["none", "mrn"] as const;
export const patientRefModeSchema = z.enum(PATIENT_REF_MODES);
export type PatientRefMode = z.infer<typeof patientRefModeSchema>;

export const PAPERS = ["a4", "letter"] as const;
export const paperSchema = z.enum(PAPERS);
export type Paper = z.infer<typeof paperSchema>;

export const WORKLIST_ASSIGNED = ["me", "unassigned", "any"] as const;
export const worklistAssignedSchema = z.enum(WORKLIST_ASSIGNED);
export type WorklistAssigned = z.infer<typeof worklistAssignedSchema>;

/** SIGN-5: shown in the sign dialog; the backend stores it with each signature. Equals `SIGNATURE_MEANING` in Rust. */
export const SIGNATURE_MEANING = "I have reviewed this observation record and approve this review.";

/** CW-1 item: the session summary plus its clinical review state. Never carries impression or MRN (RBAC-4). */
export const worklistItemSchema = sessionSummarySchema.extend({
  age_hours: z.number(),
  assigned_to: str,
  clinical_state: clinicalStateSchema.nullable().optional(),
  cosign_required: z.boolean(),
  has_patient_ref: z.boolean(),
  review_id: str,
});
export type WorklistItem = z.infer<typeof worklistItemSchema>;

export const worklistPageSchema = z.object({
  items: z.array(worklistItemSchema),
  next_cursor: z.string().nullable().optional(),
  total: z.number().int().nullable().optional(),
});
export type WorklistPage = z.infer<typeof worklistPageSchema>;

export interface WorklistQuery {
  state?: ClinicalState;
  flag?: string;
  assigned?: WorklistAssigned;
  from?: string;
  to?: string;
  cursor?: string;
  limit?: number;
}

export const clinicalReviewVersionSchema = z.object({
  version_no: z.number().int().min(0),
  status: reviewVersionStatusSchema,
  created_at: z.string(),
  created_by: str,
  disposition: dispositionSchema.nullable().optional(),
  /** Absent without `ClinicalContentRead` (RBAC-4). */
  impression: str,
  recommendations: str,
  amend_reason: str,
  signed_at: str,
});
export type ClinicalReviewVersion = z.infer<typeof clinicalReviewVersionSchema>;

export const clinicalSignatureSchema = z.object({
  id: z.string(),
  version_no: z.number().int().min(0),
  role: signatureRoleSchema,
  signer_id: z.string(),
  signer_display_name: z.string(),
  signer_registration: str,
  meaning: z.string(),
  content_hash: z.string(),
  signed_at: z.string(),
});
export type ClinicalSignature = z.infer<typeof clinicalSignatureSchema>;

export const clinicalReviewSchema = z.object({
  id: z.string(),
  org_id: z.string(),
  session_id: z.string(),
  state: clinicalStateSchema,
  assigned_to: str,
  cosign_required: z.boolean(),
  /** Sent back as `If-Match: "<row_version>"` on draft save (SIGN-3). */
  row_version: z.number().int().min(0),
  current: clinicalReviewVersionSchema,
  /** Hex SHA-256 computed by the backend (SIGN-4); displayed and echoed, never computed here. */
  content_hash: z.string(),
  signatures: z.array(clinicalSignatureSchema),
  created_at: z.string(),
  updated_at: z.string(),
});
export type ClinicalReview = z.infer<typeof clinicalReviewSchema>;

export const claimClinicalReviewRequestSchema = z.object({ assigned_to: str });
export type ClaimClinicalReviewRequest = z.infer<typeof claimClinicalReviewRequestSchema>;

export const updateClinicalReviewRequestSchema = z.object({
  assigned_to: str,
  disposition: dispositionSchema.nullable().optional(),
  impression: str,
  recommendations: str,
});
export type UpdateClinicalReviewRequest = z.infer<typeof updateClinicalReviewRequestSchema>;

/** Step-up signing body. The password exists only inside the server action and is never stored or logged. */
export const signRequestSchema = z.object({
  content_hash: z.string(),
  password: z.string(),
  meaning_ack: z.boolean(),
  totp_code: str,
});
export type SignRequest = z.infer<typeof signRequestSchema>;

export const amendClinicalReviewRequestSchema = z.object({ reason: z.string() });
export type AmendClinicalReviewRequest = z.infer<typeof amendClinicalReviewRequestSchema>;

export const userProfileSchema = z.object({
  id: z.string(),
  email: z.string(),
  role: roleSchema,
  org_id: str,
  display_name: str,
  professional_registration: str,
});
export type UserProfile = z.infer<typeof userProfileSchema>;

export const updateProfileRequestSchema = z.object({
  display_name: str,
  professional_registration: str,
});
export type UpdateProfileRequest = z.infer<typeof updateProfileRequestSchema>;

/** The clinician-workspace slice of `PATCH /orgs/{id}` `settings` (RFC 7396 merge patch; absent = unchanged). */
export interface ClinicalSettingsPatch {
  timezone?: string;
  paper?: Paper;
  identity?: { patient_ref_mode?: PatientRefMode };
  clinical?: { worklist_scope?: WorklistScope; cosign_policy?: CosignPolicy };
}

export interface ClinicalApi {
  getWorklist(orgId: string, query?: WorklistQuery): Promise<WorklistPage>;
  /** `null` when nobody has claimed the session yet (the backend answers 404). */
  getSessionClinicalReview(sessionId: string): Promise<ClinicalReview | null>;
  getClinicalReview(reviewId: string): Promise<ClinicalReview>;
  claimClinicalReview(sessionId: string, request?: ClaimClinicalReviewRequest): Promise<ClinicalReview>;
  saveClinicalDraft(reviewId: string, rowVersion: number, patch: UpdateClinicalReviewRequest): Promise<ClinicalReview>;
  signClinicalReview(reviewId: string, body: SignRequest): Promise<ClinicalReview>;
  cosignClinicalReview(reviewId: string, body: SignRequest): Promise<ClinicalReview>;
  amendClinicalReview(reviewId: string, request: AmendClinicalReviewRequest): Promise<ClinicalReview>;
  getMyProfile(): Promise<UserProfile>;
  updateMyProfile(patch: UpdateProfileRequest): Promise<UserProfile>;
  updateOrgClinicalSettings(orgId: string, settings: ClinicalSettingsPatch): Promise<Org>;
}

/** The request function `live.ts` injects (keeps `fetch` in one module). */
export type LiveRequest = <T>(
  path: string,
  schema: z.ZodType<T> | null,
  init?: { method?: string; body?: unknown; headers?: Record<string, string> },
) => Promise<T>;

export function createLiveClinical(request: LiveRequest, buildQuery: (q: object) => string): ClinicalApi {
  const enc = encodeURIComponent;
  const post = (path: string, body: unknown) => request(path, clinicalReviewSchema, { method: "POST", body });
  return {
    getWorklist: (orgId, query) =>
      request(`/orgs/${enc(orgId)}/worklist${buildQuery(query ?? {})}`, worklistPageSchema),
    async getSessionClinicalReview(sessionId) {
      try {
        return await request(`/sessions/${enc(sessionId)}/clinical-review`, clinicalReviewSchema);
      } catch (err) {
        if (err instanceof ApiError && err.status === 404) return null;
        throw err;
      }
    },
    getClinicalReview: (id) => request(`/clinical-reviews/${enc(id)}`, clinicalReviewSchema),
    claimClinicalReview: (sessionId, body) => post(`/sessions/${enc(sessionId)}/clinical-review`, body ?? {}),
    saveClinicalDraft: (id, rowVersion, patch) =>
      request(`/clinical-reviews/${enc(id)}`, clinicalReviewSchema, {
        method: "PATCH",
        body: patch,
        headers: { "If-Match": `"${rowVersion}"` },
      }),
    signClinicalReview: (id, body) => post(`/clinical-reviews/${enc(id)}/sign`, body),
    cosignClinicalReview: (id, body) => post(`/clinical-reviews/${enc(id)}/cosign`, body),
    amendClinicalReview: (id, body) => post(`/clinical-reviews/${enc(id)}/amend`, body),
    getMyProfile: () => request("/users/me", userProfileSchema),
    updateMyProfile: (patch) => request("/users/me", userProfileSchema, { method: "PATCH", body: patch }),
    updateOrgClinicalSettings: (orgId, settings) =>
      request(`/orgs/${enc(orgId)}`, orgSchema, { method: "PATCH", body: { settings } }),
  };
}
