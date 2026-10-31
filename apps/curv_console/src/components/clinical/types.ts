import type { ClinicalReview, UpdateClinicalReviewRequest } from "@/lib/api/clinical";

/** Result of a clinical server action. Carries a problem slug, never the request body (no password echo). */
export interface ResultFailure {
  slug: string | null;
  status: number;
}
export type ReviewResult = { ok: true; review: ClinicalReview } | ({ ok: false } & ResultFailure);

/** The server actions the review panel calls, already bound to the org and session. */
export interface ReviewActions {
  claim: () => Promise<ReviewResult>;
  save: (reviewId: string, rowVersion: number, patch: UpdateClinicalReviewRequest) => Promise<ReviewResult>;
  /** `FormData` holds `password`, `totp_code`, `content_hash`, `meaning_ack`; it exists only for this call. */
  sign: (reviewId: string, formData: FormData) => Promise<ReviewResult>;
  cosign: (reviewId: string, formData: FormData) => Promise<ReviewResult>;
  amend: (reviewId: string, reason: string) => Promise<ReviewResult>;
  reload: (reviewId: string) => Promise<ReviewResult>;
}
