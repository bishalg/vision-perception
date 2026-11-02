/**
 * EMR-lite record (REC-1..10; WP-C.18, UI-5): zod schemas for the `api-v1.json` `$defs` of `asd_contract::emr`, the
 * `EmrApi` interface and its live implementation. The console holds no record logic: the server mints tokens and the
 * QR payload, sanitises file names, encrypts summaries and attachment bytes and decides who may read them. The limits
 * below only size inputs and give early feedback; the backend validates again.
 *
 * Attachment bytes do not go through `EmrApi` (its live transport is JSON): `attachmentFile.ts` streams them through
 * the `app/api/attachments/...` route handlers (server actions cap bodies at 1 MB).
 */
import { z } from "@/lib/zod";
import { participantSchema } from "./admin";
import type { LiveRequest } from "./clinical";
import { timelinePageSchema, type TimelineEntry } from "./participant";
import type { PageQuery } from "./types";

const str = z.string().nullable().optional();
const pageFields = {
  next_cursor: z.string().nullable().optional(),
  total: z.number().int().min(0).nullable().optional(),
};

/** `asd_contract::emr` limits (REC-3, REC-6). */
export const MAX_ENCOUNTER_REASON_CHARS = 200;
export const MAX_ENCOUNTER_SUMMARY_CHARS = 8_000;
export const MAX_ATTACHMENT_BYTES = 10_485_760;
export const MAX_ATTACHMENT_FILE_NAME_CHARS = 120;

export const ENCOUNTER_KINDS = ["screening_visit", "follow_up_visit", "phone_call", "home_visit", "other"] as const;
export const encounterKindSchema = z.enum(ENCOUNTER_KINDS);
export type EncounterKind = z.infer<typeof encounterKindSchema>;

export const ENCOUNTER_STATUSES = ["open", "closed"] as const;
export const encounterStatusSchema = z.enum(ENCOUNTER_STATUSES);
export type EncounterStatus = z.infer<typeof encounterStatusSchema>;

export const ATTACHMENT_KINDS = ["paper_form", "referral", "report", "photo_of_document", "other"] as const;
export const attachmentKindSchema = z.enum(ATTACHMENT_KINDS);
export type AttachmentKind = z.infer<typeof attachmentKindSchema>;

export const ATTACHMENT_MEDIA_TYPES = ["application/pdf", "image/jpeg", "image/png"] as const;
export const attachmentMediaTypeSchema = z.enum(ATTACHMENT_MEDIA_TYPES);
export type AttachmentMediaType = z.infer<typeof attachmentMediaTypeSchema>;

// ---- participants: create and search (REC-1, REC-2) ---------------------------

export const createParticipantRequestSchema = z.object({
  age_bracket_at_enrol: str,
  language: str,
  consent_version: str,
  consent_at: str,
});
export type CreateParticipantRequest = z.infer<typeof createParticipantRequestSchema>;

/** `201` of `POST /orgs/{id}/participants`: the flattened `Participant` plus the server-built QR payload. */
export const createdParticipantSchema = participantSchema.extend({ qr_payload: z.string() });
export type CreatedParticipant = z.infer<typeof createdParticipantSchema>;

/** Every filter optional; `mrn` (with `PatientRefRead`) only ever travels in this POST body (MRN-4, UI-4). */
export const participantSearchRequestSchema = z.object({
  token_prefix: str,
  mrn: str,
  age_bracket: str,
  last_seen_from: str,
  last_seen_to: str,
  has_open_follow_up: z.boolean().nullable().optional(),
  cursor: str,
  limit: z.number().int().min(0).nullable().optional(),
});
export type ParticipantSearchRequest = z.infer<typeof participantSearchRequestSchema>;

/** One search row: the flattened `Participant` plus the record counters (REC-2). */
export const participantSearchRowSchema = participantSchema.extend({
  open_encounters_n: z.number().int().min(0).optional(),
  next_follow_up_at: str,
});
export type ParticipantSearchRow = z.infer<typeof participantSearchRowSchema>;

export const participantSearchPageSchema = z.object({ items: z.array(participantSearchRowSchema), ...pageFields });
export type ParticipantSearchPage = z.infer<typeof participantSearchPageSchema>;

// ---- encounters and follow-ups (REC-3, REC-4) ---------------------------------

export const encounterSchema = z.object({
  id: z.string(),
  participant_id: z.string(),
  kind: encounterKindSchema,
  occurred_at: z.string(),
  clinician_id: str,
  clinician_display_name: str,
  reason: str,
  /** Absent without `ClinicalContentRead`; `has_summary` says whether one exists. */
  summary: str,
  has_summary: z.boolean(),
  status: encounterStatusSchema,
  follow_up_due_at: str,
  follow_up_done_at: str,
  session_ids: z.array(z.string()).optional(),
  /** Sent back as `If-Match: "<row_version>"` on `PATCH` (`412 stale-version`). */
  row_version: z.number().int(),
  created_at: z.string(),
  updated_at: str,
});
export type Encounter = z.infer<typeof encounterSchema>;

export const encounterPageSchema = z.object({ items: z.array(encounterSchema), ...pageFields });
export type EncounterPage = z.infer<typeof encounterPageSchema>;

export const createEncounterRequestSchema = z.object({
  kind: encounterKindSchema,
  occurred_at: z.string(),
  clinician_id: str,
  reason: str,
  /** Needs `ClinicalNoteWrite`, else `400 encounter-summary-forbidden`. */
  summary: str,
  follow_up_due_at: str,
  session_ids: z.array(z.string()).optional(),
});
export type CreateEncounterRequest = z.infer<typeof createEncounterRequestSchema>;

/** Absent fields stay unchanged; a closed encounter accepts only `follow_up_done_at` and reopening. */
export const updateEncounterRequestSchema = z.object({
  kind: encounterKindSchema.nullable().optional(),
  occurred_at: str,
  clinician_id: str,
  reason: str,
  summary: str,
  status: encounterStatusSchema.nullable().optional(),
  follow_up_due_at: str,
  follow_up_done_at: str,
});
export type UpdateEncounterRequest = z.infer<typeof updateEncounterRequestSchema>;

export const linkSessionRequestSchema = z.object({ session_id: z.string() });
export type LinkSessionRequest = z.infer<typeof linkSessionRequestSchema>;

/** An open follow-up: an encounter with a due date and no done date. Never carries the summary. */
export const followUpSchema = z.object({
  encounter_id: z.string(),
  participant_id: z.string(),
  participant_token: z.string(),
  kind: encounterKindSchema,
  occurred_at: z.string(),
  follow_up_due_at: z.string(),
  clinician_id: str,
  clinician_display_name: str,
  reason: str,
});
export type FollowUp = z.infer<typeof followUpSchema>;

export const followUpPageSchema = z.object({ items: z.array(followUpSchema), ...pageFields });
export type FollowUpPage = z.infer<typeof followUpPageSchema>;

/** `GET /orgs/{id}/follow-ups?due_before=&assigned=`: `due_before` an RFC 3339 instant, `assigned=me` mine only. */
export interface FollowUpQuery extends PageQuery {
  due_before?: string;
  assigned?: "me";
}

// ---- attachments (REC-6, REC-7) -------------------------------------------------

export const attachmentSchema = z.object({
  id: z.string(),
  participant_id: z.string(),
  encounter_id: str,
  kind: attachmentKindSchema,
  file_name: z.string(),
  media_type: attachmentMediaTypeSchema,
  byte_size: z.number().int().min(0),
  /** Hex SHA-256 of the plaintext; `null` until the bytes are uploaded. */
  sha256: str,
  uploaded_by: str,
  created_at: z.string(),
});
export type Attachment = z.infer<typeof attachmentSchema>;

export const createAttachmentRequestSchema = z.object({
  kind: attachmentKindSchema,
  file_name: z.string(),
  media_type: attachmentMediaTypeSchema,
  byte_size: z.number().int().min(0),
  encounter_id: str,
});
export type CreateAttachmentRequest = z.infer<typeof createAttachmentRequestSchema>;

export const uploadTargetSchema = z.object({ method: z.string(), url: z.string() });
export type UploadTarget = z.infer<typeof uploadTargetSchema>;

export const attachmentUploadSchema = z.object({ attachment: attachmentSchema, upload: uploadTargetSchema });
export type AttachmentUpload = z.infer<typeof attachmentUploadSchema>;

/**
 * One row of the Attachments tab. The contract has no attachment list route: the rows are the participant's
 * `attachment` timeline entries (REC-5 header: kind, name, size), so every role that reads the timeline sees the
 * same list and only the download (`ClinicalContentRead`) is restricted.
 */
export interface AttachmentRow {
  id: string;
  at: string;
  kind: AttachmentKind | null;
  file_name: string | null;
  byte_size: number | null;
  uploaded_by: string | null;
}

/** REC-5 attachment header -> row. Tolerates `kind`/`attachment_kind`, `file_name`/`name`, `byte_size`/`size`. */
export function attachmentRowFromTimeline(e: TimelineEntry): AttachmentRow {
  const s = e.summary;
  const pick = (...keys: string[]) => keys.map((k) => s[k]).find((v) => v !== undefined && v !== null);
  const kind = attachmentKindSchema.safeParse(pick("attachment_kind", "kind"));
  const name = pick("file_name", "name");
  const size = pick("byte_size", "size");
  return {
    id: e.id,
    at: e.at,
    kind: kind.success ? kind.data : null,
    file_name: typeof name === "string" ? name : null,
    byte_size: typeof size === "number" && Number.isFinite(size) ? size : null,
    uploaded_by: e.actor_display_name ?? null,
  };
}

export interface EmrApi {
  /** REC-1 (`ParticipantCreate`): the token and QR payload are minted by the server. `201`. */
  createParticipant(orgId: string, request: CreateParticipantRequest): Promise<CreatedParticipant>;
  /** REC-2: POST body only, so an MRN never reaches a URL or a log line. */
  searchParticipants(orgId: string, request: ParticipantSearchRequest): Promise<ParticipantSearchPage>;
  /** Newest `occurred_at` first. `summary` only for `ClinicalContentRead`; the read is audited when it is returned. */
  listEncounters(participantId: string, query?: PageQuery): Promise<EncounterPage>;
  getEncounter(encounterId: string): Promise<Encounter>;
  createEncounter(participantId: string, request: CreateEncounterRequest): Promise<Encounter>;
  /** `If-Match: "<row_version>"`; `412 stale-version` when someone saved first, `409 encounter-closed`. */
  updateEncounter(encounterId: string, rowVersion: number, patch: UpdateEncounterRequest): Promise<Encounter>;
  linkEncounterSession(encounterId: string, sessionId: string): Promise<void>;
  unlinkEncounterSession(encounterId: string, sessionId: string): Promise<void>;
  /** Open follow-ups, earliest due first. */
  listFollowUps(orgId: string, query?: FollowUpQuery): Promise<FollowUpPage>;
  /** Metadata first (`201` with the upload target); the bytes follow through `/api/attachments/{id}/content`. */
  createAttachment(participantId: string, request: CreateAttachmentRequest): Promise<AttachmentUpload>;
  deleteAttachment(attachmentId: string): Promise<void>;
  /** The participant's attachments from the timeline (`attachment` headers), newest first. */
  listParticipantAttachments(participantId: string): Promise<AttachmentRow[]>;
}

export function createLiveEmr(request: LiveRequest, buildQuery: (q: object) => string): EmrApi {
  const enc = encodeURIComponent;
  return {
    createParticipant: (orgId, body) =>
      request(`/orgs/${enc(orgId)}/participants`, createdParticipantSchema, { method: "POST", body }),
    searchParticipants: (orgId, body) =>
      request(`/orgs/${enc(orgId)}/participants/search`, participantSearchPageSchema, { method: "POST", body }),
    listEncounters: (id, query) =>
      request(`/participants/${enc(id)}/encounters${buildQuery(query ?? {})}`, encounterPageSchema),
    getEncounter: (id) => request(`/encounters/${enc(id)}`, encounterSchema),
    createEncounter: (id, body) =>
      request(`/participants/${enc(id)}/encounters`, encounterSchema, { method: "POST", body }),
    updateEncounter: (id, rowVersion, patch) =>
      request(`/encounters/${enc(id)}`, encounterSchema, {
        method: "PATCH",
        body: patch,
        headers: { "If-Match": `"${rowVersion}"` },
      }),
    // The answer body (if any) is not needed: the page reloads the encounter list.
    linkEncounterSession: (id, sessionId) =>
      request(`/encounters/${enc(id)}/sessions`, null, { method: "POST", body: { session_id: sessionId } }),
    unlinkEncounterSession: (id, sessionId) =>
      request(`/encounters/${enc(id)}/sessions/${enc(sessionId)}`, null, { method: "DELETE" }),
    listFollowUps: (orgId, query) =>
      request(`/orgs/${enc(orgId)}/follow-ups${buildQuery(query ?? {})}`, followUpPageSchema),
    createAttachment: (id, body) =>
      request(`/participants/${enc(id)}/attachments`, attachmentUploadSchema, { method: "POST", body }),
    deleteAttachment: (id) => request(`/attachments/${enc(id)}`, null, { method: "DELETE" }),
    async listParticipantAttachments(id) {
      const rows: AttachmentRow[] = [];
      let cursor: string | undefined;
      // Bounded: a participant with more than 500 attachments shows the newest 500.
      for (let i = 0; i < 5; i++) {
        const page = await request(
          `/participants/${enc(id)}/timeline${buildQuery({ kind: "attachment", limit: 100, cursor })}`,
          timelinePageSchema,
        );
        rows.push(...page.items.filter((e) => e.kind === "attachment").map(attachmentRowFromTimeline));
        cursor = page.next_cursor ?? undefined;
        if (!cursor) break;
      }
      return rows;
    },
  };
}
