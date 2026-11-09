"use server";

import { revalidatePath } from "next/cache";
import type { ActionFailure, ActionResult } from "@/components/exports/types";
import { ApiError } from "@/lib/api";
import { exportModeSchema, exportPurposeSchema, type ExportRequest } from "@/lib/api/exports";
import { authedApi } from "@/lib/auth/server";
import { pathsFor } from "@/lib/paths";

function failure(err: unknown): ActionFailure {
  if (err instanceof ApiError) {
    const params = err.problem?.invalid_params?.map((p) => `${p.name}: ${p.reason}`).join("; ");
    return { ok: false, slug: err.slug, status: err.status, ...(params ? { params } : {}) };
  }
  return { ok: false, slug: null, status: 0 };
}

const text = (fd: FormData, k: string) => String(fd.get(k) ?? "").trim();

/** `YYYY-MM-DD` from a date input → RFC 3339 at UTC midnight; anything else is sent as typed for the backend to judge. */
function consentInstant(day: string): string | undefined {
  if (!day) return undefined;
  return /^\d{4}-\d{2}-\d{2}$/.test(day) ? `${day}T00:00:00Z` : day;
}

/**
 * WP-C.19 (SHR-7, SHR-10): "Share case" submits one `case_package` export for this participant. The console checks
 * nothing the backend does not (UI-2): the mode, purpose, consent and role rules are the backend's, and its answer is
 * shown as is.
 */
export async function createCaseShareAction(
  orgId: string,
  participantId: string,
  formData: FormData,
): Promise<ActionResult<{ id: string }>> {
  const mode = exportModeSchema.safeParse(text(formData, "mode"));
  const purpose = exportPurposeSchema.safeParse(text(formData, "purpose"));
  if (!mode.success || !purpose.success) return { ok: false, slug: "validation-failed", status: 400 };
  const identified = mode.data === "identified";
  const sessions = formData.getAll("session_ids").map(String).filter(Boolean);
  const consentReference = text(formData, "consent_reference");
  const consentAt = consentInstant(text(formData, "consent_recorded_at"));
  const request: ExportRequest = {
    format: "case_package",
    mode: mode.data,
    purpose: purpose.data,
    reason: text(formData, "reason"),
    case: {
      participant_id: participantId,
      recipient: text(formData, "recipient"),
      ...(sessions.length ? { session_ids: sessions } : {}),
      ...(identified && consentReference ? { consent_reference: consentReference } : {}),
      ...(identified && consentAt ? { consent_recorded_at: consentAt } : {}),
      ...(identified ? { include_attachments: formData.get("include_attachments") !== null } : {}),
    },
  };
  try {
    const created = await (await authedApi()).createExport(orgId, request);
    revalidatePath(pathsFor(orgId).data.exports);
    return { ok: true, id: created.id };
  } catch (err) {
    return failure(err);
  }
}

export type CaseSessionsResult = { ok: true; sessions: { id: string; startedAt: string }[] } | ActionFailure;

/** The participant's sessions for the dialog's optional session picker (newest first, up to 100). */
export async function caseSessionsAction(orgId: string, participantId: string): Promise<CaseSessionsResult> {
  try {
    const page = await (await authedApi()).listSessions(orgId, { participant_id: participantId, limit: 100 });
    return { ok: true, sessions: page.items.map((s) => ({ id: s.id, startedAt: s.started_at })) };
  } catch (err) {
    return failure(err);
  }
}
