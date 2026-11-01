/**
 * Participant workspace API (WP-C.14; UI-5): zod schemas for the DTOs of `docs/spec/schemas/asd/api-v1.json`
 * (participant detail, timeline, clinical notes, MRN mapping, disclosures), the `ParticipantApi` interface and the
 * live HTTP implementation. The console holds no crypto, hashing or MRN normalisation: the MRN is sent in a POST or
 * PUT body only (UI-4) and the backend decides everything else.
 */
import { z } from "zod";
import { participantSchema } from "./admin";
import type { LiveRequest } from "./clinical";
import {
  deliveryStatusSchema,
  exportFormatSchema,
  exportKindSchema,
  exportModeSchema,
  exportPurposeSchema,
  exportStatusSchema,
} from "./exports";
import { ApiError, type PageQuery } from "./types";

const str = z.string().nullable().optional();

export const NOTE_KINDS = ["general", "correction"] as const;
export const noteKindSchema = z.enum(NOTE_KINDS);
export type NoteKind = z.infer<typeof noteKindSchema>;

export const TIMELINE_KINDS = ["session", "clinical_review", "note", "letter", "care_event"] as const;
export const timelineKindSchema = z.enum(TIMELINE_KINDS);
export type TimelineKind = z.infer<typeof timelineKindSchema>;

/** NOTE-1 / MRN-3 limits, mirrored from `asd_contract::clinical` only to size inputs; the backend validates. */
export const MAX_NOTE_CHARS = 20_000;
export const MAX_ASSIGNING_AUTHORITY_CHARS = 40;
export const MAX_MRN_CHARS = 64;

/** `GET /participants/{id}`: the same `Participant` as the list items. */
export const participantDetailSchema = participantSchema;

/** Kind-specific header, never free text, e.g. `{"attention_flag": "red"}` or `{"state": "final", "version_no": 2}`. */
export const timelineEntrySchema = z.object({
  kind: timelineKindSchema,
  id: z.string(),
  at: z.string(),
  session_id: str,
  actor_id: str,
  actor_display_name: str,
  summary: z.record(z.string(), z.unknown()),
});
export type TimelineEntry = z.infer<typeof timelineEntrySchema>;

export const timelinePageSchema = z.object({
  items: z.array(timelineEntrySchema),
  next_cursor: z.string().nullable().optional(),
});
export type TimelinePage = z.infer<typeof timelinePageSchema>;

export const clinicalNoteSchema = z.object({
  id: z.string(),
  participant_id: z.string(),
  session_id: str,
  kind: noteKindSchema,
  /** Absent without `ClinicalContentRead`. */
  body: str,
  corrects_id: str,
  /** Id of the note that corrects this one (shown struck through). */
  corrected_by: str,
  author_id: z.string(),
  author_display_name: str,
  created_at: z.string(),
});
export type ClinicalNote = z.infer<typeof clinicalNoteSchema>;

export const clinicalNotePageSchema = z.object({
  items: z.array(clinicalNoteSchema),
  next_cursor: z.string().nullable().optional(),
  total: z.number().int().min(0).nullable().optional(),
});
export type ClinicalNotePage = z.infer<typeof clinicalNotePageSchema>;

/** Notes are append-only (NOTE-2): a correction is a new note with `corrects_id`. */
export const createClinicalNoteRequestSchema = z.object({
  body: z.string(),
  session_id: str,
  corrects_id: str,
});
export type CreateClinicalNoteRequest = z.infer<typeof createClinicalNoteRequestSchema>;

export const patientRefSchema = z.object({
  participant_id: z.string(),
  assigning_authority: z.string(),
  mrn: z.string(),
  updated_at: z.string(),
});
export type PatientRef = z.infer<typeof patientRefSchema>;

export const putPatientRefRequestSchema = z.object({ assigning_authority: z.string(), mrn: z.string() });
export type PutPatientRefRequest = z.infer<typeof putPatientRefRequestSchema>;

/** `PUT` answers the authority and time, never the MRN (the caller just sent it). */
export const patientRefStoredSchema = z.object({
  participant_id: z.string(),
  assigning_authority: z.string(),
  updated_at: z.string(),
});
export type PatientRefStored = z.infer<typeof patientRefStoredSchema>;

export const patientRefSearchRequestSchema = z.object({ assigning_authority: z.string(), mrn: z.string() });
export type PatientRefSearchRequest = z.infer<typeof patientRefSearchRequestSchema>;

export const patientRefSearchResponseSchema = z.object({ participant_id: str });
export type PatientRefSearchResponse = z.infer<typeof patientRefSearchResponseSchema>;

export const disclosureDeliverySchema = z.object({
  destination_id: z.string(),
  destination_name: z.string(),
  status: deliveryStatusSchema,
  delivered_at: str,
});
export type DisclosureDelivery = z.infer<typeof disclosureDeliverySchema>;

/** One export that included the participant (accounting of disclosures, EXJ-6). */
export const disclosureSchema = z.object({
  export_id: z.string(),
  kind: exportKindSchema,
  format: exportFormatSchema,
  mode: exportModeSchema,
  purpose: exportPurposeSchema,
  status: exportStatusSchema,
  requested_by: str,
  created_at: z.string(),
  session_ids: z.array(z.string()),
  deliveries: z.array(disclosureDeliverySchema),
});
export type Disclosure = z.infer<typeof disclosureSchema>;

export const disclosurePageSchema = z.object({
  items: z.array(disclosureSchema),
  next_cursor: z.string().nullable().optional(),
  total: z.number().int().min(0).nullable().optional(),
});
export type DisclosurePage = z.infer<typeof disclosurePageSchema>;

export interface ParticipantApi {
  getParticipant(participantId: string): Promise<z.infer<typeof participantDetailSchema>>;
  /** Newest first, headers only (CW-3). */
  getParticipantTimeline(participantId: string, query?: PageQuery): Promise<TimelinePage>;
  listParticipantNotes(participantId: string, query?: PageQuery): Promise<ClinicalNotePage>;
  createParticipantNote(participantId: string, request: CreateClinicalNoteRequest): Promise<ClinicalNote>;
  /** `null` when no MRN is linked (404). Throws 409 `patient-ref-disabled` when the org does not use MRNs. Audited. */
  getPatientRef(participantId: string): Promise<PatientRef | null>;
  putPatientRef(participantId: string, request: PutPatientRefRequest): Promise<PatientRefStored>;
  deletePatientRef(participantId: string): Promise<void>;
  searchPatientRef(orgId: string, request: PatientRefSearchRequest): Promise<PatientRefSearchResponse>;
  listParticipantDisclosures(participantId: string, query?: PageQuery): Promise<DisclosurePage>;
}

export function createLiveParticipant(request: LiveRequest, buildQuery: (q: object) => string): ParticipantApi {
  const enc = encodeURIComponent;
  return {
    getParticipant: (id) => request(`/participants/${enc(id)}`, participantDetailSchema),
    getParticipantTimeline: (id, query) =>
      request(`/participants/${enc(id)}/timeline${buildQuery(query ?? {})}`, timelinePageSchema),
    listParticipantNotes: (id, query) =>
      request(`/participants/${enc(id)}/notes${buildQuery(query ?? {})}`, clinicalNotePageSchema),
    createParticipantNote: (id, body) =>
      request(`/participants/${enc(id)}/notes`, clinicalNoteSchema, { method: "POST", body }),
    async getPatientRef(id) {
      try {
        return await request(`/participants/${enc(id)}/patient-ref`, patientRefSchema);
      } catch (err) {
        if (err instanceof ApiError && err.status === 404) return null;
        throw err;
      }
    },
    putPatientRef: (id, body) =>
      request(`/participants/${enc(id)}/patient-ref`, patientRefStoredSchema, { method: "PUT", body }),
    deletePatientRef: (id) => request(`/participants/${enc(id)}/patient-ref`, null, { method: "DELETE" }),
    searchPatientRef: (orgId, body) =>
      request(`/orgs/${enc(orgId)}/patient-refs/search`, patientRefSearchResponseSchema, { method: "POST", body }),
    listParticipantDisclosures: (id, query) =>
      request(`/participants/${enc(id)}/disclosures${buildQuery(query ?? {})}`, disclosurePageSchema),
  };
}
