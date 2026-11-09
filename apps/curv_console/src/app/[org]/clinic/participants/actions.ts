"use server";

import { revalidatePath } from "next/cache";
import type { ActionFailure, ActionResult } from "@/components/participant/types";
import { ApiError, type ParticipantSearchPage } from "@/lib/api";
import { authedApi } from "@/lib/auth/server";
import { pathsFor } from "@/lib/paths";

function failure(err: unknown): ActionFailure {
  if (err instanceof ApiError) {
    const params = err.problem?.invalid_params?.map((p) => `${p.name}: ${p.reason}`).join("; ");
    return { ok: false, slug: err.slug, status: err.status, ...(params ? { params } : {}) };
  }
  return { ok: false, slug: null, status: 0 };
}

const text = (form: FormData, name: string): string => {
  const v = form.get(name);
  return typeof v === "string" ? v.trim() : "";
};

/** `YYYY-MM-DD` (a date picker) -> the start (or end) of that UTC day; anything else is dropped. */
function day(raw: string, edge: "start" | "end"): string | undefined {
  if (!/^\d{4}-\d{2}-\d{2}$/.test(raw)) return undefined;
  const t = Date.parse(`${raw}T${edge === "start" ? "00:00:00.000" : "23:59:59.999"}Z`);
  return Number.isNaN(t) ? undefined : new Date(t).toISOString();
}

export interface CreatedView {
  id: string;
  participant_token: string;
  qr_payload: string;
}

/**
 * POST /v1/orgs/{id}/participants (REC-1). The server mints the token and the QR payload; the console only shows them.
 * No identity fields exist before wave 3.
 */
export async function createParticipantAction(orgId: string, form: FormData): Promise<ActionResult<{ participant: CreatedView }>> {
  const consentDay = text(form, "consent_date");
  try {
    const p = await (await authedApi()).createParticipant(orgId, {
      ...(text(form, "age_bracket") ? { age_bracket_at_enrol: text(form, "age_bracket") } : {}),
      ...(text(form, "language") ? { language: text(form, "language") } : {}),
      ...(text(form, "consent_version") ? { consent_version: text(form, "consent_version") } : {}),
      ...(consentDay && day(consentDay, "start") ? { consent_at: day(consentDay, "start") } : {}),
    });
    revalidatePath(pathsFor(orgId).clinic.participants);
    return { ok: true, participant: { id: p.id, participant_token: p.participant_token, qr_payload: p.qr_payload } };
  } catch (err) {
    return failure(err);
  }
}

/**
 * POST /v1/orgs/{id}/participants/search (REC-2, UI-4): the filters, MRN included, travel only in this POST body and
 * the answer goes back to the page as data; nothing reaches the address bar. `cursor` continues the same search.
 */
export async function searchParticipantsAction(orgId: string, form: FormData): Promise<ActionResult<{ page: ParticipantSearchPage }>> {
  const mrn = text(form, "mrn");
  const cursor = text(form, "cursor");
  try {
    const page = await (await authedApi()).searchParticipants(orgId, {
      ...(text(form, "token_prefix") ? { token_prefix: text(form, "token_prefix") } : {}),
      ...(text(form, "age_bracket") ? { age_bracket: text(form, "age_bracket") } : {}),
      ...(day(text(form, "last_seen_from"), "start") ? { last_seen_from: day(text(form, "last_seen_from"), "start") } : {}),
      ...(day(text(form, "last_seen_to"), "end") ? { last_seen_to: day(text(form, "last_seen_to"), "end") } : {}),
      ...(form.get("has_open_follow_up") === "true" ? { has_open_follow_up: true } : {}),
      ...(mrn ? { mrn } : {}),
      ...(cursor ? { cursor } : {}),
      limit: 25,
    });
    return { ok: true, page };
  } catch (err) {
    return failure(err);
  }
}
