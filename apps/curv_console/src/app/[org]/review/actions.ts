"use server";

import { revalidatePath } from "next/cache";
import { ApiError } from "@/lib/api";
import { authedApi } from "@/lib/auth/server";
import { isCaptureIssueLabel, isTriageAction, triagePatch, type CaptureIssueLabel, type TriageAction } from "@/lib/review";
import type { TriageResult } from "@/components/ReviewQueue";

function message(err: unknown): string {
  if (err instanceof ApiError) return err.slug ?? `status ${err.status}`;
  return "unavailable";
}

/** Apply one triage action (spec 03 §4.2) via PATCH /v1/review/{id}. The backend checks org and role. */
export async function triageAction(
  orgId: string,
  reviewId: string,
  action: TriageAction,
  label?: CaptureIssueLabel,
): Promise<TriageResult> {
  if (!isTriageAction(action)) return { ok: false, error: "unknown action" };
  if (action === "capture_issue" && !isCaptureIssueLabel(label)) return { ok: false, error: "missing label" };
  try {
    const api = await authedApi();
    // The backend checks that the item belongs to the caller's org (404 otherwise).
    const item = await api.updateReview(reviewId, triagePatch(action, label));
    const base = `/${encodeURIComponent(orgId)}`;
    revalidatePath(`${base}/review`);
    revalidatePath(`${base}/overview`);
    return { ok: true, item };
  } catch (err) {
    return { ok: false, error: message(err) };
  }
}
