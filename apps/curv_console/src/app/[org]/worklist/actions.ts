"use server";

import { revalidatePath } from "next/cache";
import type { ReviewResult } from "@/components/clinical/types";
import { ApiError } from "@/lib/api";
import {
  paperSchema,
  cosignPolicySchema,
  patientRefModeSchema,
  worklistScopeSchema,
  type ClinicalSettingsPatch,
  type UpdateClinicalReviewRequest,
} from "@/lib/api/clinical";
import { authedApi } from "@/lib/auth/server";

/** Failure outcome: the problem slug and status only. Never the request body, never a password. */
function failure(err: unknown): ReviewResult {
  if (err instanceof ApiError) return { ok: false, slug: err.slug, status: err.status };
  return { ok: false, slug: null, status: 0 };
}

function touch(orgId: string): void {
  revalidatePath(`/${encodeURIComponent(orgId)}/worklist`);
}

export async function claimReviewAction(orgId: string, sessionId: string): Promise<ReviewResult> {
  try {
    const review = await (await authedApi()).claimClinicalReview(sessionId, {});
    touch(orgId);
    return { ok: true, review };
  } catch (err) {
    return failure(err);
  }
}

export async function saveDraftAction(
  orgId: string,
  reviewId: string,
  rowVersion: number,
  patch: UpdateClinicalReviewRequest,
): Promise<ReviewResult> {
  try {
    const review = await (await authedApi()).saveClinicalDraft(reviewId, rowVersion, patch);
    touch(orgId);
    return { ok: true, review };
  } catch (err) {
    return failure(err);
  }
}

/** The only place the password is read: it goes straight into the API request body and is dropped. */
function signBody(formData: FormData) {
  const totp = String(formData.get("totp_code") ?? "").trim();
  return {
    content_hash: String(formData.get("content_hash") ?? ""),
    password: String(formData.get("password") ?? ""),
    meaning_ack: formData.get("meaning_ack") === "true",
    ...(totp ? { totp_code: totp } : {}),
  };
}

export async function signReviewAction(orgId: string, reviewId: string, formData: FormData): Promise<ReviewResult> {
  try {
    const review = await (await authedApi()).signClinicalReview(reviewId, signBody(formData));
    touch(orgId);
    return { ok: true, review };
  } catch (err) {
    return failure(err);
  }
}

export async function cosignReviewAction(orgId: string, reviewId: string, formData: FormData): Promise<ReviewResult> {
  try {
    const review = await (await authedApi()).cosignClinicalReview(reviewId, signBody(formData));
    touch(orgId);
    return { ok: true, review };
  } catch (err) {
    return failure(err);
  }
}

export async function amendReviewAction(orgId: string, reviewId: string, reason: string): Promise<ReviewResult> {
  try {
    const review = await (await authedApi()).amendClinicalReview(reviewId, { reason });
    touch(orgId);
    return { ok: true, review };
  } catch (err) {
    return failure(err);
  }
}

export async function reloadReviewAction(reviewId: string): Promise<ReviewResult> {
  try {
    return { ok: true, review: await (await authedApi()).getClinicalReview(reviewId) };
  } catch (err) {
    return failure(err);
  }
}

// ---- settings ---------------------------------------------------------------------------------------------------

export type SettingsResult = { ok: true } | { ok: false; slug: string | null; status: number; params?: string };

function settingsFailure(err: unknown): SettingsResult {
  if (err instanceof ApiError) {
    const params = err.problem?.invalid_params?.map((p) => `${p.name}: ${p.reason}`).join("; ");
    return { ok: false, slug: err.slug, status: err.status, ...(params ? { params } : {}) };
  }
  return { ok: false, slug: null, status: 0 };
}

/** Builds a merge patch from the form: an empty selection means "leave unchanged" (the current values are not readable). */
export async function saveClinicalSettingsAction(orgId: string, formData: FormData): Promise<SettingsResult> {
  const text = (k: string) => String(formData.get(k) ?? "").trim();
  const patch: ClinicalSettingsPatch = {};
  const scope = worklistScopeSchema.safeParse(text("worklist_scope"));
  const cosign = cosignPolicySchema.safeParse(text("cosign_policy"));
  const mode = patientRefModeSchema.safeParse(text("patient_ref_mode"));
  const paper = paperSchema.safeParse(text("paper"));
  if (scope.success || cosign.success) {
    patch.clinical = {
      ...(scope.success ? { worklist_scope: scope.data } : {}),
      ...(cosign.success ? { cosign_policy: cosign.data } : {}),
    };
  }
  if (mode.success) patch.identity = { patient_ref_mode: mode.data };
  if (paper.success) patch.paper = paper.data;
  if (text("timezone")) patch.timezone = text("timezone");
  if (Object.keys(patch).length === 0) return { ok: true };
  try {
    await (await authedApi()).updateOrgClinicalSettings(orgId, patch);
    revalidatePath(`/${encodeURIComponent(orgId)}/settings/clinical`);
    touch(orgId);
    return { ok: true };
  } catch (err) {
    return settingsFailure(err);
  }
}

/** Signer profile (NOTE-6): empty string clears a field, per the contract. */
export async function saveProfileAction(orgId: string, formData: FormData): Promise<SettingsResult> {
  try {
    await (await authedApi()).updateMyProfile({
      display_name: String(formData.get("display_name") ?? "").trim(),
      professional_registration: String(formData.get("professional_registration") ?? "").trim(),
    });
    revalidatePath(`/${encodeURIComponent(orgId)}/settings/clinical`);
    return { ok: true };
  } catch (err) {
    return settingsFailure(err);
  }
}
