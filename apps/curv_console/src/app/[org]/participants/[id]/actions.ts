"use server";

import { revalidatePath } from "next/cache";
import { redirect } from "next/navigation";
import type { ActionFailure, ActionResult, RevealResult } from "@/components/participant/types";
import { ApiError } from "@/lib/api";
import { authedApi } from "@/lib/auth/server";

function failure(err: unknown): ActionFailure {
  if (err instanceof ApiError) {
    const params = err.problem?.invalid_params?.map((p) => `${p.name}: ${p.reason}`).join("; ");
    return { ok: false, slug: err.slug, status: err.status, ...(params ? { params } : {}) };
  }
  return { ok: false, slug: null, status: 0 };
}

const text = (form: FormData, name: string): string => {
  const v = form.get(name);
  return typeof v === "string" ? v : "";
};

const page = (orgId: string, participantId: string) =>
  `/${encodeURIComponent(orgId)}/participants/${encodeURIComponent(participantId)}`;

/** POST /v1/participants/{id}/notes. The note text is sent once and never returned or put in a URL (UI-4). */
export async function createNoteAction(orgId: string, participantId: string, form: FormData): Promise<ActionResult> {
  try {
    const session = text(form, "session_id");
    const corrects = text(form, "corrects_id");
    await (await authedApi()).createParticipantNote(participantId, {
      body: text(form, "body"),
      ...(session ? { session_id: session } : {}),
      ...(corrects ? { corrects_id: corrects } : {}),
    });
    revalidatePath(page(orgId, participantId));
    return { ok: true };
  } catch (err) {
    return failure(err);
  }
}

/** GET /v1/participants/{id}/patient-ref: the decrypt is audited by the backend, so it only runs on request. */
export async function revealMrnAction(participantId: string): Promise<RevealResult> {
  try {
    const ref = await (await authedApi()).getPatientRef(participantId);
    if (!ref) return { ok: false, slug: "not-found", status: 404 };
    return { ok: true, authority: ref.assigning_authority, mrn: ref.mrn, updatedAt: ref.updated_at };
  } catch (err) {
    return failure(err);
  }
}

/** PUT /v1/participants/{id}/patient-ref. */
export async function saveMrnAction(orgId: string, participantId: string, form: FormData): Promise<ActionResult> {
  try {
    await (await authedApi()).putPatientRef(participantId, {
      assigning_authority: text(form, "assigning_authority"),
      mrn: text(form, "mrn"),
    });
    revalidatePath(page(orgId, participantId));
    return { ok: true };
  } catch (err) {
    return failure(err);
  }
}

/** DELETE /v1/participants/{id}/patient-ref. */
export async function unlinkMrnAction(orgId: string, participantId: string): Promise<ActionResult> {
  try {
    await (await authedApi()).deletePatientRef(participantId);
    revalidatePath(page(orgId, participantId));
    return { ok: true };
  } catch (err) {
    return failure(err);
  }
}

/**
 * POST /v1/orgs/{id}/patient-refs/search. A hit redirects to that participant's MRN tab (the address holds the
 * participant id, never the MRN); a miss returns `found: false`.
 */
export async function searchMrnAction(orgId: string, form: FormData): Promise<ActionResult<{ found: boolean }>> {
  let id: string | null | undefined;
  try {
    const r = await (await authedApi()).searchPatientRef(orgId, {
      assigning_authority: text(form, "assigning_authority"),
      mrn: text(form, "mrn"),
    });
    id = r.participant_id;
  } catch (err) {
    return failure(err);
  }
  if (id) redirect(`${page(orgId, id)}?tab=mrn`);
  return { ok: true, found: false };
}
