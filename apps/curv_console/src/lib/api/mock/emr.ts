/**
 * Mock EMR-lite record (WP-C.18): participant create and search, encounters, follow-ups and attachments. Synthetic
 * data only. Like the other mocks it has one caller identity, so role rules (who may write a summary or read an
 * attachment) live in the UI helpers; the API-shaped rules (row versions, closed encounters, sizes, media types,
 * `Content-Digest`) are enforced here like the backend.
 *
 * The record shares its participants with the admin mock: `createMockAdmin` creates the store with its participant
 * list and registers it (`registerMockRecordStore`); `createMockEmr`, called next in the same `createMockClient`, takes
 * it, and the participant mock finds it by its `listParticipants` function to add `encounter` / `attachment` timeline
 * entries. Wave 2 step 0 gave `createMockEmr` only `{orgId, userId, now}`, and `mock.ts` is not WP-C.18's to edit; at
 * integration this hand-off can become an explicit context field.
 */
import type { Participant, ParticipantPage } from "../admin";
import type { MockAttachmentContent } from "../attachmentFile";
import {
  ATTACHMENT_KINDS,
  ATTACHMENT_MEDIA_TYPES,
  ENCOUNTER_KINDS,
  ENCOUNTER_STATUSES,
  MAX_ATTACHMENT_BYTES,
  MAX_ATTACHMENT_FILE_NAME_CHARS,
  MAX_ENCOUNTER_REASON_CHARS,
  MAX_ENCOUNTER_SUMMARY_CHARS,
  type Attachment,
  type EmrApi,
  type Encounter,
  type FollowUp,
  type ParticipantSearchRow,
} from "../emr";
import type { TimelineEntry } from "../participant";
import { ApiError, PROBLEM_BASE, type PageQuery, type SessionDetail } from "../types";

export interface MockEmrContext {
  orgId: string;
  userId: string;
  /** Epoch milliseconds (`MOCK_FIXED_NOW`). */
  now: number;
}

type PageFn = <T>(
  rows: T[],
  key: (row: T) => [string, string],
  cursor: string | undefined,
  limit?: number,
) => { slice: T[]; next_cursor: string | null; total: number };

type StoredAttachment = Attachment & { deleted_at: string | null; bytes: Uint8Array | null };

/** State shared by the admin, participant and record mocks of one mock client. */
export interface MockRecordStore {
  orgId: string;
  userId: string;
  now: number;
  details: SessionDetail[];
  /** The admin mock's participant rows (newest first), mutated in place on create. */
  participants: (Participant & { created_at: string })[];
  page: PageFn;
  encounters: Encounter[];
  attachments: StoredAttachment[];
  seeded: boolean;
}

const MIN = 60_000;
const DAY = 86_400_000;
const CLINICIAN = "Dr. Demo Clinician";

let pending: MockRecordStore | undefined;
const byListFn = new WeakMap<object, MockRecordStore>();

/** Called by `createMockAdmin`: the next `createMockEmr` takes this store, and the participant mock finds it by `listFn`. */
export function registerMockRecordStore(store: MockRecordStore, listFn: (orgId: string, query?: PageQuery) => Promise<ParticipantPage>): void {
  pending = store;
  byListFn.set(listFn, store);
}

/** `encounter` and `attachment` timeline headers (REC-5) for the participant mock; empty without a registered store. */
export function mockRecordTimeline(listFn: object, participantId: string): TimelineEntry[] {
  const store = byListFn.get(listFn);
  if (!store) return [];
  seed(store);
  const entries: TimelineEntry[] = [];
  for (const e of store.encounters) {
    if (e.participant_id !== participantId) continue;
    entries.push({
      kind: "encounter",
      id: e.id,
      at: e.occurred_at,
      session_id: null,
      actor_id: e.clinician_id ?? null,
      actor_display_name: e.clinician_display_name ?? null,
      summary: { encounter_kind: e.kind, status: e.status },
    });
  }
  for (const a of store.attachments) {
    if (a.participant_id !== participantId || a.deleted_at) continue;
    entries.push({
      kind: "attachment",
      id: a.id,
      at: a.created_at,
      session_id: null,
      actor_id: a.uploaded_by ?? null,
      actor_display_name: CLINICIAN,
      summary: { attachment_kind: a.kind, file_name: a.file_name, byte_size: a.byte_size },
    });
  }
  return entries;
}

function problem(status: number, slug: string, title: string, detail?: string, params?: [string, string][]): ApiError {
  return new ApiError(status, {
    type: `${PROBLEM_BASE}${slug}`,
    title,
    status,
    detail,
    ...(params ? { invalid_params: params.map(([name, reason]) => ({ name, reason })) } : {}),
  });
}
const invalid = (name: string, reason: string) => problem(400, "validation-failed", "Validation failed", undefined, [[name, reason]]);
const notFound = () => problem(404, "not-found", "Not found");
const iso = (ms: number) => new Date(ms).toISOString();
const chars = (s: string) => [...s].length;

/** Deterministic ids per store, in UUID shape. */
function ids(prefix: string) {
  let n = 0;
  return () => `${prefix}-0000-4000-8000-${String(++n).padStart(12, "0")}`;
}
const nextEncounterId = new WeakMap<MockRecordStore, () => string>();
const nextAttachmentId = new WeakMap<MockRecordStore, () => string>();
const idFor = (map: WeakMap<MockRecordStore, () => string>, store: MockRecordStore, prefix: string) => {
  let f = map.get(store);
  if (!f) map.set(store, (f = ids(prefix)));
  return f();
};

/** A 1x1 PNG, so a seeded attachment downloads as a real image. */
const PNG_1X1 = Uint8Array.from(
  atob("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg=="),
  (c) => c.charCodeAt(0),
);
const PNG_1X1_SHA256 = "6b7fa434f92a8b80aab02d9bf1a12e49ffcae424e4013a1c4f68b67e3d2bbcd0";

function seed(store: MockRecordStore): void {
  if (store.seeded) return;
  store.seeded = true;
  // The newest fixture participants (with sessions) carry a visit history; one has an overdue follow-up.
  const withSessions = store.participants.filter((p) => store.details.some((d) => d.participant_id === p.id));
  withSessions.slice(0, 4).forEach((p, k) => {
    const sessions = store.details.filter((d) => d.participant_id === p.id);
    const first = sessions[sessions.length - 1];
    const at = first ? Date.parse(first.started_at) : store.now - 20 * DAY;
    const base = {
      participant_id: p.id,
      clinician_id: store.userId,
      clinician_display_name: CLINICIAN,
      follow_up_done_at: null,
      created_at: iso(at),
      updated_at: null,
      row_version: 1,
    };
    store.encounters.push({
      ...base,
      id: idFor(nextEncounterId, store, "e0c00000"),
      kind: "screening_visit",
      occurred_at: iso(at - 30 * MIN),
      reason: "First observation visit",
      summary: "Calm during the session. Parent asked about sleep routines.",
      has_summary: true,
      status: "closed",
      follow_up_due_at: null,
      session_ids: first ? [first.id] : [],
    });
    // Due dates: overdue, today, next week, next month.
    const due = [store.now - 1 * DAY, store.now + 2 * 3_600_000, store.now + 7 * DAY, store.now + 30 * DAY][k]!;
    store.encounters.push({
      ...base,
      id: idFor(nextEncounterId, store, "e0c00000"),
      kind: k % 2 ? "phone_call" : "follow_up_visit",
      occurred_at: iso(store.now - (k + 2) * DAY),
      reason: k % 2 ? "Check on home routine" : "Repeat observation",
      summary: null,
      has_summary: false,
      status: "open",
      follow_up_due_at: iso(due),
      session_ids: [],
    });
  });
  const firstP = withSessions[0];
  if (firstP) {
    store.attachments.push({
      id: idFor(nextAttachmentId, store, "a77ac000"),
      participant_id: firstP.id,
      encounter_id: store.encounters[0]?.id ?? null,
      kind: "paper_form",
      file_name: "consent-form-scan.png",
      media_type: "image/png",
      byte_size: PNG_1X1.byteLength,
      // The bytes are here, so (as on the backend) the row carries their hash; `null` means "not uploaded yet".
      sha256: PNG_1X1_SHA256,
      uploaded_by: store.userId,
      created_at: iso(store.now - 10 * DAY),
      deleted_at: null,
      bytes: PNG_1X1,
    });
  }
}

/** Store for a standalone `createMockEmr` (unit tests): no participants until created. */
function emptyStore(ctx: MockEmrContext): MockRecordStore {
  const page: PageFn = (rows, key, cursor, limit = 20) => {
    const size = Math.min(Math.max(limit, 1), 100);
    const start = cursor ? rows.findIndex((r) => key(r).join("|") === cursor) + 1 : 0;
    if (cursor && start === 0) throw invalid("cursor", "invalid");
    const slice = rows.slice(start, start + size);
    const last = slice[slice.length - 1];
    return { slice, next_cursor: start + size < rows.length && last ? key(last).join("|") : null, total: rows.length };
  };
  return { ...ctx, details: [], participants: [], page, encounters: [], attachments: [], seeded: false };
}

async function sha256(bytes: Uint8Array): Promise<Uint8Array> {
  return new Uint8Array(await globalThis.crypto.subtle.digest("SHA-256", bytes as BufferSource));
}
const toHex = (b: Uint8Array) => Array.from(b, (x) => x.toString(16).padStart(2, "0")).join("");
const toB64 = (b: Uint8Array) => btoa(String.fromCharCode(...b));

/** RFC 9530 `sha-256=:<base64>:` member of a `Content-Digest` header, or `undefined` when malformed. */
function digestMember(header: string): string | undefined {
  for (const part of header.split(",")) {
    const m = part.trim().match(/^sha-256=:([A-Za-z0-9+/]+={0,2}):$/);
    if (m) return m[1];
  }
  return undefined;
}

/** Mirrors `asd_contract::emr::sanitize_file_name` so mock rows look like the server's. */
function sanitize(name: string): string {
  const out = name.replace(/[^A-Za-z0-9._-]/g, "_").replace(/^\.+/, "");
  return [...out].slice(0, MAX_ATTACHMENT_FILE_NAME_CHARS).join("") || "file";
}

function randomHex(n: number): string {
  const bytes = new Uint8Array(Math.ceil(n / 2));
  globalThis.crypto.getRandomValues(bytes);
  return toHex(bytes).slice(0, n);
}

export function createMockEmr(ctx: MockEmrContext): EmrApi & MockAttachmentContent {
  const store = pending ?? emptyStore(ctx);
  pending = undefined;
  let seq = 0;
  const stamp = () => iso(store.now + ++seq * MIN);

  const ensureOrg = (orgId: string) => {
    if (orgId !== store.orgId) throw notFound();
  };
  const participant = (id: string) => {
    seed(store);
    const p = store.participants.find((x) => x.id === id);
    if (!p) throw notFound();
    return p;
  };
  const encounter = (id: string) => {
    seed(store);
    const e = store.encounters.find((x) => x.id === id);
    if (!e) throw notFound();
    return e;
  };
  const attachment = (id: string) => {
    seed(store);
    const a = store.attachments.find((x) => x.id === id);
    if (!a || a.deleted_at) throw notFound();
    return a;
  };
  const sessionsOf = (participantId: string) => store.details.filter((d) => d.participant_id === participantId);
  const time = (name: string, raw: string | null | undefined) => {
    if (raw == null || raw === "") return null;
    const t = Date.parse(raw);
    if (Number.isNaN(t)) throw invalid(name, "must be an RFC 3339 timestamp");
    return iso(t);
  };
  const text = (name: string, raw: string | null | undefined, max: number) => {
    if (raw == null) return null;
    const v = raw.trim();
    if (chars(v) > max) throw invalid(name, `must be at most ${max} characters`);
    return v || null;
  };
  const openFollowUp = (e: Encounter) => Boolean(e.follow_up_due_at) && !e.follow_up_done_at;
  const copy = (e: Encounter): Encounter => ({ ...e, session_ids: [...(e.session_ids ?? [])] });

  return {
    async createParticipant(orgId, req) {
      ensureOrg(orgId);
      seed(store);
      const age = text("age_bracket_at_enrol", req.age_bracket_at_enrol, 16);
      const language = text("language", req.language, 16);
      text("consent_version", req.consent_version, 64);
      time("consent_at", req.consent_at);
      const token = `child-${randomHex(20)}`;
      const row = {
        id: `c4ea7e00-0000-4000-8000-${randomHex(12)}`,
        org_id: store.orgId,
        participant_token: token,
        age_bracket_at_enrol: age,
        language,
        session_count: 0,
        last_session_at: null,
        withdrawn_at: null,
        has_patient_ref: false,
        created_at: stamp(),
      };
      store.participants.unshift(row);
      const out: Participant & { created_at?: string } = { ...row };
      delete out.created_at;
      return { ...out, qr_payload: `asd://participant/${token}?org=${store.orgId}` };
    },

    async searchParticipants(orgId, req) {
      ensureOrg(orgId);
      seed(store);
      if (req.mrn != null && req.mrn.trim() === "") throw invalid("mrn", "must not be empty");
      const prefix = req.token_prefix?.trim().toLowerCase();
      const from = time("last_seen_from", req.last_seen_from);
      const to = time("last_seen_to", req.last_seen_to);
      const rows: (ParticipantSearchRow & { created_at: string })[] = store.participants
        .map((p) => {
          const open = store.encounters.filter((e) => e.participant_id === p.id && e.status === "open");
          const due = store.encounters
            .filter((e) => e.participant_id === p.id && openFollowUp(e))
            .map((e) => e.follow_up_due_at!)
            .sort()[0];
          return { ...p, open_encounters_n: open.length, next_follow_up_at: due ?? null };
        })
        .filter(
          (p) =>
            // The mock keeps MRNs in the participant mock; an MRN search finds nothing here.
            req.mrn == null &&
            (!prefix || p.participant_token.toLowerCase().startsWith(prefix)) &&
            (!req.age_bracket || p.age_bracket_at_enrol === req.age_bracket) &&
            (!from || (p.last_session_at != null && p.last_session_at >= from)) &&
            (!to || (p.last_session_at != null && p.last_session_at <= to)) &&
            (req.has_open_follow_up == null || Boolean(p.next_follow_up_at) === req.has_open_follow_up),
        );
      const { slice, next_cursor, total } = store.page(rows, (p) => [p.created_at, p.id], req.cursor ?? undefined, req.limit ?? undefined);
      return {
        items: slice.map((p) => {
          const out: ParticipantSearchRow & { created_at?: string } = { ...p };
          delete out.created_at;
          return out;
        }),
        next_cursor,
        total,
      };
    },

    async listEncounters(participantId, query = {}) {
      participant(participantId);
      const rows = store.encounters
        .filter((e) => e.participant_id === participantId)
        .sort((a, b) => b.occurred_at.localeCompare(a.occurred_at) || b.id.localeCompare(a.id));
      const { slice, next_cursor, total } = store.page(rows, (e) => [e.occurred_at, e.id], query.cursor, query.limit);
      return { items: slice.map(copy), next_cursor, total };
    },

    async getEncounter(id) {
      return copy(encounter(id));
    },

    async createEncounter(participantId, req) {
      const p = participant(participantId);
      if (p.withdrawn_at) throw problem(409, "participant-withdrawn", "Conflict");
      if (!(ENCOUNTER_KINDS as readonly string[]).includes(req.kind)) throw invalid("kind", "unknown encounter kind");
      const occurred = time("occurred_at", req.occurred_at);
      if (!occurred) throw invalid("occurred_at", "is required");
      const sessions = req.session_ids ?? [];
      for (const s of sessions) {
        if (!sessionsOf(participantId).some((d) => d.id === s)) throw invalid("session_ids", "is not a session of this participant");
      }
      const summary = text("summary", req.summary, MAX_ENCOUNTER_SUMMARY_CHARS);
      const e: Encounter = {
        id: idFor(nextEncounterId, store, "e0c00000"),
        participant_id: participantId,
        kind: req.kind,
        occurred_at: occurred,
        clinician_id: req.clinician_id ?? store.userId,
        clinician_display_name: CLINICIAN,
        reason: text("reason", req.reason, MAX_ENCOUNTER_REASON_CHARS),
        summary,
        has_summary: summary != null,
        status: "open",
        follow_up_due_at: time("follow_up_due_at", req.follow_up_due_at),
        follow_up_done_at: null,
        session_ids: [...sessions],
        row_version: 1,
        created_at: stamp(),
        updated_at: null,
      };
      store.encounters.push(e);
      return copy(e);
    },

    async updateEncounter(id, rowVersion, patch) {
      const e = encounter(id);
      if (rowVersion !== e.row_version) {
        throw problem(412, "stale-version", "Precondition failed", "the encounter was changed by someone else");
      }
      if (patch.status != null && !(ENCOUNTER_STATUSES as readonly string[]).includes(patch.status)) {
        throw invalid("status", "must be open or closed");
      }
      if (patch.kind != null && !(ENCOUNTER_KINDS as readonly string[]).includes(patch.kind)) throw invalid("kind", "unknown encounter kind");
      if (e.status === "closed") {
        const other = Object.entries(patch).some(
          ([k, v]) => v != null && k !== "follow_up_done_at" && !(k === "status" && (v === "open" || v === "closed")),
        );
        if (other) throw problem(409, "encounter-closed", "Conflict", "a closed visit accepts only the follow-up done date or reopening");
      }
      if (patch.kind != null) e.kind = patch.kind;
      if (patch.occurred_at != null) e.occurred_at = time("occurred_at", patch.occurred_at)!;
      // As `PATCH /v1/encounters/{id}` (B.19): an absent or `null` field is kept, `""` clears an optional one.
      if (patch.clinician_id != null) e.clinician_id = patch.clinician_id || null;
      if (patch.reason != null) e.reason = text("reason", patch.reason, MAX_ENCOUNTER_REASON_CHARS);
      if (patch.summary != null) {
        e.summary = text("summary", patch.summary, MAX_ENCOUNTER_SUMMARY_CHARS);
        e.has_summary = e.summary != null;
      }
      if (patch.follow_up_due_at != null) e.follow_up_due_at = time("follow_up_due_at", patch.follow_up_due_at);
      if (patch.follow_up_done_at != null) e.follow_up_done_at = time("follow_up_done_at", patch.follow_up_done_at);
      if (patch.status != null) e.status = patch.status;
      e.row_version += 1;
      e.updated_at = stamp();
      return copy(e);
    },

    async linkEncounterSession(id, sessionId) {
      const e = encounter(id);
      if (e.status === "closed") throw problem(409, "encounter-closed", "Conflict", "a closed visit accepts only the follow-up done date or reopening");
      if (!sessionsOf(e.participant_id).some((d) => d.id === sessionId)) {
        throw invalid("session_id", "is not a session of this participant");
      }
      if (!e.session_ids?.includes(sessionId)) e.session_ids = [...(e.session_ids ?? []), sessionId];
    },

    async unlinkEncounterSession(id, sessionId) {
      const e = encounter(id);
      if (e.status === "closed") throw problem(409, "encounter-closed", "Conflict", "a closed visit accepts only the follow-up done date or reopening");
      if (!e.session_ids?.includes(sessionId)) throw notFound();
      e.session_ids = e.session_ids.filter((s) => s !== sessionId);
    },

    async listFollowUps(orgId, query = {}) {
      ensureOrg(orgId);
      seed(store);
      const before = time("due_before", query.due_before);
      const rows: FollowUp[] = store.encounters
        .filter((e) => openFollowUp(e))
        .filter((e) => !before || e.follow_up_due_at! <= before)
        .filter((e) => query.assigned !== "me" || e.clinician_id === store.userId)
        .flatMap((e) => {
          const p = store.participants.find((x) => x.id === e.participant_id);
          return p
            ? [{
                encounter_id: e.id,
                participant_id: e.participant_id,
                participant_token: p.participant_token,
                kind: e.kind,
                occurred_at: e.occurred_at,
                follow_up_due_at: e.follow_up_due_at!,
                clinician_id: e.clinician_id ?? null,
                clinician_display_name: e.clinician_display_name ?? null,
                reason: e.reason ?? null,
              }]
            : [];
        })
        .sort((a, b) => a.follow_up_due_at.localeCompare(b.follow_up_due_at) || a.encounter_id.localeCompare(b.encounter_id));
      const { slice, next_cursor, total } = store.page(rows, (f) => [f.follow_up_due_at, f.encounter_id], query.cursor, query.limit);
      return { items: slice, next_cursor, total };
    },

    async createAttachment(participantId, req) {
      const p = participant(participantId);
      if (p.withdrawn_at) throw problem(409, "participant-withdrawn", "Conflict");
      if (!(ATTACHMENT_KINDS as readonly string[]).includes(req.kind)) throw invalid("kind", "unknown attachment kind");
      if (!(ATTACHMENT_MEDIA_TYPES as readonly string[]).includes(req.media_type)) {
        throw problem(400, "attachment-type-not-allowed", "Bad request", "only PDF, JPEG and PNG files are accepted");
      }
      if (!Number.isInteger(req.byte_size) || req.byte_size < 1 || req.byte_size > MAX_ATTACHMENT_BYTES) {
        throw problem(400, "attachment-too-large", "Bad request", "files must be 1 byte to 10 MiB");
      }
      if (req.encounter_id && !store.encounters.some((e) => e.id === req.encounter_id && e.participant_id === participantId)) {
        throw invalid("encounter_id", "is not a visit of this participant");
      }
      const a: StoredAttachment = {
        id: `a77ac000-0000-4000-8000-${randomHex(12)}`,
        participant_id: participantId,
        encounter_id: req.encounter_id ?? null,
        kind: req.kind,
        file_name: sanitize(req.file_name),
        media_type: req.media_type,
        byte_size: req.byte_size,
        sha256: null,
        uploaded_by: store.userId,
        created_at: stamp(),
        deleted_at: null,
        bytes: null,
      };
      store.attachments.push(a);
      const { deleted_at: _d, bytes: _b, ...meta } = a;
      void _d;
      void _b;
      return { attachment: meta, upload: { method: "PUT", url: `/v1/attachments/${a.id}/content` } };
    },

    async deleteAttachment(id) {
      attachment(id).deleted_at = stamp();
    },

    async listAttachments(participantId, query: PageQuery = {}) {
      participant(participantId);
      // As the backend: live rows newest first, including ones whose bytes have not arrived yet (`sha256: null`).
      const rows = store.attachments
        .filter((a) => a.participant_id === participantId && !a.deleted_at)
        .sort((a, b) => b.created_at.localeCompare(a.created_at) || b.id.localeCompare(a.id));
      const { slice, next_cursor, total } = store.page(rows, (a) => [a.created_at, a.id], query.cursor, query.limit);
      const items = slice.map((a): Attachment => {
        const { deleted_at: _d, bytes: _b, ...meta } = a;
        void _d;
        void _b;
        return meta;
      });
      return { items, next_cursor, total };
    },

    async mockPutAttachmentContent(id, bytes, mediaType, digest) {
      const a = attachment(id);
      if (bytes.byteLength > MAX_ATTACHMENT_BYTES) throw problem(400, "attachment-too-large", "Bad request");
      if (mediaType !== a.media_type) throw problem(400, "attachment-type-not-allowed", "Bad request");
      if (bytes.byteLength !== a.byte_size) throw invalid("byte_size", "does not match the uploaded content");
      const hash = await sha256(bytes);
      if (digest != null) {
        const claimed = digestMember(digest);
        if (!claimed) throw invalid("Content-Digest", "must carry a sha-256=:<base64>: member");
        if (claimed !== toB64(hash)) throw problem(400, "digest-mismatch", "Bad request");
      }
      a.bytes = bytes.slice();
      a.sha256 = toHex(hash);
    },

    async mockGetAttachmentContent(id) {
      const a = attachment(id);
      if (!a.bytes) throw notFound();
      return { bytes: a.bytes.slice(), media_type: a.media_type, file_name: a.file_name };
    },
  };
}
