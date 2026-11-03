"use server";

import { revalidatePath } from "next/cache";
import { redirect } from "next/navigation";
import type { ActionFailure, ActionResult, RevealResult } from "@/components/participant/types";
import {
  ApiError,
  MAX_ATTACHMENT_BYTES,
  attachmentKindSchema,
  attachmentMediaTypeSchema,
  encounterKindSchema,
} from "@/lib/api";
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

// ---- visits (REC-4) --------------------------------------------------------------

const optional = (form: FormData, name: string): string | undefined => {
  const v = text(form, name).trim();
  return v ? v : undefined;
};

/** The client sends `occurred_at` / `follow_up_due_at` as RFC 3339 instants it built from the date pickers. */
function encounterFields(form: FormData) {
  const kind = encounterKindSchema.safeParse(text(form, "kind"));
  return {
    kind: kind.success ? kind.data : undefined,
    occurred_at: optional(form, "occurred_at"),
    reason: text(form, "reason").trim(),
    // Present only when the form had a summary field (roles with `note.write`); absent = no summary sent at all.
    summary: form.has("summary") ? text(form, "summary") : undefined,
    follow_up_due_at: optional(form, "follow_up_due_at"),
  };
}

/** POST /v1/participants/{id}/encounters. The summary is sent once and never returned in a URL or title (UI-4). */
export async function createEncounterAction(orgId: string, participantId: string, form: FormData): Promise<ActionResult> {
  const f = encounterFields(form);
  if (!f.kind) return { ok: false, slug: "validation-failed", status: 400, params: "kind: choose the type of visit" };
  if (!f.occurred_at) return { ok: false, slug: "validation-failed", status: 400, params: "occurred_at: choose the date" };
  const sessions = form.getAll("session_ids").filter((v): v is string => typeof v === "string" && v !== "");
  try {
    await (await authedApi()).createEncounter(participantId, {
      kind: f.kind,
      occurred_at: f.occurred_at,
      ...(f.reason ? { reason: f.reason } : {}),
      ...(f.summary?.trim() ? { summary: f.summary } : {}),
      ...(f.follow_up_due_at ? { follow_up_due_at: f.follow_up_due_at } : {}),
      ...(sessions.length ? { session_ids: sessions } : {}),
    });
    revalidatePath(page(orgId, participantId));
    return { ok: true };
  } catch (err) {
    return failure(err);
  }
}

/**
 * PATCH /v1/encounters/{id} with `If-Match`: kind, date, reason, follow-up date and (for clinicians) the summary.
 * The backend keeps a field that is absent or `null` and clears an optional one on `""`, so an emptied reason,
 * summary or follow-up date is sent as `""`.
 */
export async function updateEncounterAction(
  orgId: string,
  participantId: string,
  encounterId: string,
  rowVersion: number,
  form: FormData,
): Promise<ActionResult> {
  const f = encounterFields(form);
  try {
    await (await authedApi()).updateEncounter(encounterId, rowVersion, {
      ...(f.kind ? { kind: f.kind } : {}),
      ...(f.occurred_at ? { occurred_at: f.occurred_at } : {}),
      reason: f.reason,
      ...(f.summary !== undefined ? { summary: f.summary } : {}),
      follow_up_due_at: f.follow_up_due_at ?? "",
    });
    revalidatePath(page(orgId, participantId));
    return { ok: true };
  } catch (err) {
    return failure(err);
  }
}

/** Close or reopen a visit, or mark its follow-up done (server time). */
export async function encounterStateAction(
  orgId: string,
  participantId: string,
  encounterId: string,
  rowVersion: number,
  change: "close" | "reopen" | "follow_up_done",
): Promise<ActionResult> {
  const patch =
    change === "follow_up_done"
      ? { follow_up_done_at: new Date().toISOString() }
      : { status: change === "close" ? ("closed" as const) : ("open" as const) };
  try {
    await (await authedApi()).updateEncounter(encounterId, rowVersion, patch);
    revalidatePath(page(orgId, participantId));
    return { ok: true };
  } catch (err) {
    return failure(err);
  }
}

/** POST / DELETE /v1/encounters/{id}/sessions: link or unlink a session of the same participant. */
export async function encounterSessionAction(
  orgId: string,
  participantId: string,
  encounterId: string,
  sessionId: string,
  link: boolean,
): Promise<ActionResult> {
  try {
    const api = await authedApi();
    if (link) await api.linkEncounterSession(encounterId, sessionId);
    else await api.unlinkEncounterSession(encounterId, sessionId);
    revalidatePath(page(orgId, participantId));
    return { ok: true };
  } catch (err) {
    return failure(err);
  }
}

// ---- files (REC-6, REC-7) --------------------------------------------------------

export interface AttachmentMeta {
  kind: string;
  file_name: string;
  media_type: string;
  byte_size: number;
  encounter_id?: string;
}

/**
 * POST /v1/participants/{id}/attachments (metadata only). The browser then sends the bytes to
 * `/api/attachments/{id}/content`, a route handler, because server action bodies are capped at 1 MB.
 */
export async function createAttachmentAction(
  orgId: string,
  participantId: string,
  meta: AttachmentMeta,
): Promise<ActionResult<{ id: string }>> {
  const kind = attachmentKindSchema.safeParse(meta.kind);
  const media = attachmentMediaTypeSchema.safeParse(meta.media_type);
  if (!kind.success) return { ok: false, slug: "validation-failed", status: 400, params: "kind: choose what the file is" };
  if (!media.success) return { ok: false, slug: "attachment-type-not-allowed", status: 400 };
  if (!Number.isInteger(meta.byte_size) || meta.byte_size < 1 || meta.byte_size > MAX_ATTACHMENT_BYTES) {
    return { ok: false, slug: "attachment-too-large", status: 400 };
  }
  try {
    const created = await (await authedApi()).createAttachment(participantId, {
      kind: kind.data,
      file_name: String(meta.file_name).slice(0, 255),
      media_type: media.data,
      byte_size: meta.byte_size,
      ...(meta.encounter_id ? { encounter_id: String(meta.encounter_id) } : {}),
    });
    return { ok: true, id: created.attachment.id };
  } catch (err) {
    return failure(err);
  }
}

/** DELETE /v1/attachments/{id}; also used to drop the metadata row when the byte upload failed. */
export async function deleteAttachmentAction(orgId: string, participantId: string, attachmentId: string): Promise<ActionResult> {
  try {
    await (await authedApi()).deleteAttachment(attachmentId);
    revalidatePath(page(orgId, participantId));
    return { ok: true };
  } catch (err) {
    return failure(err);
  }
}

/** After a successful upload: refresh the workspace so the new file is listed. */
export async function attachmentUploadedAction(orgId: string, participantId: string): Promise<void> {
  revalidatePath(page(orgId, participantId));
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
