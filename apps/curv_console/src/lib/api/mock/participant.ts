/**
 * Fixture-backed participant workspace API for mock mode (WP-C.14). Synthetic data only: no real names, no real MRN.
 * Notes are append-only like the backend (NOTE-2); the MRN mapping normalises only to detect duplicates, as the blind
 * index does (MRN-3). The mock never audits and has one caller identity, so role checks live in the UI helpers.
 */
import type { Participant, ParticipantPage } from "../admin";
import {
  MAX_ASSIGNING_AUTHORITY_CHARS,
  MAX_MRN_CHARS,
  MAX_NOTE_CHARS,
  type ClinicalNote,
  type Disclosure,
  type ParticipantApi,
  type PatientRef,
  type TimelineEntry,
} from "../participant";
import { ApiError, PROBLEM_BASE, type PageQuery, type SessionDetail } from "../types";

export interface MockParticipantContext {
  details: SessionDetail[];
  orgId: string;
  userId: string;
  now: number;
  page: <T>(
    rows: T[],
    key: (row: T) => [string, string],
    cursor: string | undefined,
    limit?: number,
  ) => { slice: T[]; next_cursor: string | null; total: number };
  listParticipants: (orgId: string, query?: PageQuery) => Promise<ParticipantPage>;
}

const MIN = 60_000;
const DAY = 86_400_000;
const AUTHOR = "Dr. Demo Clinician";

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
const normalise = (s: string) => s.toLowerCase().replace(/[^a-z0-9]/g, "");

export function createMockParticipant(ctx: MockParticipantContext): ParticipantApi {
  const iso = (ms: number) => new Date(ms).toISOString();
  const notes = new Map<string, ClinicalNote[]>(); // participant id -> notes, newest first
  const refs = new Map<string, { authority: string; mrn: string; updated_at: string }>();
  const disclosures = new Map<string, Disclosure[]>();
  let participants: Participant[] | undefined;
  let seq = 0;
  const newId = () => `a11ce000-0000-4000-8000-${String(++seq).padStart(12, "0")}`;

  async function all(): Promise<Participant[]> {
    if (participants) return participants;
    const rows: Participant[] = [];
    let cursor: string | undefined;
    do {
      const page = await ctx.listParticipants(ctx.orgId, { cursor, limit: 100 });
      rows.push(...page.items);
      cursor = page.next_cursor ?? undefined;
    } while (cursor);
    participants = rows;
    seed(rows);
    return rows;
  }

  const sessionsOf = (id: string) =>
    ctx.details.filter((d) => d.participant_id === id).sort((a, b) => b.started_at.localeCompare(a.started_at));

  function seed(rows: Participant[]): void {
    rows.forEach((p, k) => {
      const sessions = sessionsOf(p.id);
      if (p.has_patient_ref) refs.set(p.id, { authority: "TUTH", mrn: `TU-${String(10_040 + k)}`, updated_at: iso(ctx.now - 5 * DAY) });
      const list: ClinicalNote[] = [];
      const last = sessions[0];
      if (last && (k === 0 || k % 2 === 1)) {
        const original = newId();
        const base = { participant_id: p.id, author_id: ctx.userId, author_display_name: AUTHOR };
        list.push({
          ...base,
          id: original,
          session_id: last.id,
          kind: "general",
          body: "Parent reports good eye contact at home. Follow up after the next capture.",
          corrects_id: null,
          corrected_by: null,
          created_at: iso(Date.parse(last.started_at) + 2 * 3_600_000),
        });
        if (k === 0) {
          const fix = newId();
          list[0]!.corrected_by = fix;
          list.unshift({
            ...base,
            id: fix,
            session_id: last.id,
            kind: "correction",
            body: "Parent reports good eye contact at home, but limited name response. Follow up after the next capture.",
            corrects_id: original,
            corrected_by: null,
            created_at: iso(Date.parse(last.started_at) + 26 * 3_600_000),
          });
        }
      }
      notes.set(p.id, list);
      if (k === 0 && last) {
        disclosures.set(p.id, [
          {
            export_id: newId(),
            kind: "bulk",
            format: "csv",
            mode: "identified",
            purpose: "clinical_care",
            status: "succeeded",
            requested_by: AUTHOR,
            created_at: iso(ctx.now - 2 * DAY),
            session_ids: sessions.slice(0, 2).map((s) => s.id),
            deliveries: [
              { destination_id: newId(), destination_name: "Hospital SFTP (test)", status: "delivered", delivered_at: iso(ctx.now - 2 * DAY + 5 * MIN) },
              { destination_id: newId(), destination_name: "Referral webhook", status: "failed", delivered_at: null },
            ],
          },
        ]);
      }
    });
  }

  async function find(id: string): Promise<Participant> {
    const p = (await all()).find((x) => x.id === id);
    if (!p) throw problem(404, "not-found", "Not found");
    return p;
  }

  return {
    async getParticipant(id) {
      const p = await find(id);
      return { ...p, has_patient_ref: refs.has(id) };
    },

    async getParticipantTimeline(id, query = {}) {
      await find(id);
      const entries: TimelineEntry[] = [];
      sessionsOf(id).forEach((s, i) => {
        entries.push({
          kind: "session",
          id: s.id,
          at: s.started_at,
          session_id: s.id,
          actor_id: null,
          actor_display_name: null,
          summary: { attention_flag: s.attention_flag },
        });
        if (s.reviewed) {
          entries.push({
            kind: "clinical_review",
            id: `c0ffee00-0000-4000-8000-${String(i + 1).padStart(12, "0")}`,
            at: iso(Date.parse(s.started_at) + 10 * MIN),
            session_id: s.id,
            actor_id: ctx.userId,
            actor_display_name: AUTHOR,
            summary: { state: "final", version_no: 1 },
          });
        }
      });
      for (const n of notes.get(id) ?? []) {
        entries.push({
          kind: "note",
          id: n.id,
          at: n.created_at,
          session_id: n.session_id ?? null,
          actor_id: n.author_id,
          actor_display_name: n.author_display_name ?? null,
          summary: { note_kind: n.kind },
        });
      }
      const first = sessionsOf(id).at(-1);
      if (first) {
        for (const [i, kind] of ["diary", "milestone"].entries()) {
          entries.push({
            kind: "care_event",
            id: `ca4e0000-0000-4000-8000-${String(i + 1).padStart(12, "0")}`,
            at: iso(Date.parse(first.started_at) + (i + 1) * DAY),
            session_id: null,
            actor_id: null,
            actor_display_name: null,
            summary: { care_kind: kind },
          });
        }
      }
      entries.sort((a, b) => b.at.localeCompare(a.at) || b.id.localeCompare(a.id));
      const { slice, next_cursor } = ctx.page(entries, (e) => [e.at, e.id], query.cursor, query.limit);
      return { items: slice, next_cursor };
    },

    async listParticipantNotes(id, query = {}) {
      await find(id);
      const rows = [...(notes.get(id) ?? [])].sort((a, b) => b.created_at.localeCompare(a.created_at) || b.id.localeCompare(a.id));
      const { slice, next_cursor, total } = ctx.page(rows, (n) => [n.created_at, n.id], query.cursor, query.limit);
      return { items: slice, next_cursor, total };
    },

    async createParticipantNote(id, req) {
      await find(id);
      if (req.body.trim() === "") throw invalid("body", "must not be empty");
      if ([...req.body].length > MAX_NOTE_CHARS) throw invalid("body", `must be at most ${MAX_NOTE_CHARS} characters`);
      if (req.session_id && !sessionsOf(id).some((s) => s.id === req.session_id)) {
        throw invalid("session_id", "is not a session of this participant");
      }
      const list = notes.get(id) ?? [];
      let target: ClinicalNote | undefined;
      if (req.corrects_id) {
        target = list.find((n) => n.id === req.corrects_id);
        if (!target) throw invalid("corrects_id", "is not a note of this participant");
      }
      const note: ClinicalNote = {
        id: newId(),
        participant_id: id,
        session_id: req.session_id ?? target?.session_id ?? null,
        kind: target ? "correction" : "general",
        body: req.body,
        corrects_id: target?.id ?? null,
        corrected_by: null,
        author_id: ctx.userId,
        author_display_name: AUTHOR,
        created_at: iso(ctx.now + ++seq * MIN),
      };
      if (target) target.corrected_by = note.id;
      notes.set(id, [note, ...list]);
      return { ...note };
    },

    async getPatientRef(id) {
      await find(id);
      const r = refs.get(id);
      if (!r) return null;
      const out: PatientRef = { participant_id: id, assigning_authority: r.authority, mrn: r.mrn, updated_at: r.updated_at };
      return out;
    },

    async putPatientRef(id, req) {
      await find(id);
      const authority = req.assigning_authority.trim().toUpperCase();
      const mrn = req.mrn.trim();
      if (!authority || !normalise(authority)) throw invalid("assigning_authority", "must not be empty");
      if ([...authority].length > MAX_ASSIGNING_AUTHORITY_CHARS) {
        throw invalid("assigning_authority", `must be at most ${MAX_ASSIGNING_AUTHORITY_CHARS} characters`);
      }
      if (!mrn || !normalise(mrn)) throw invalid("mrn", "must not be empty");
      if ([...mrn].length > MAX_MRN_CHARS) throw invalid("mrn", `must be at most ${MAX_MRN_CHARS} characters`);
      for (const [other, r] of refs) {
        if (other !== id && r.authority === authority && normalise(r.mrn) === normalise(mrn)) {
          throw problem(409, "conflict", "Conflict", "this patient reference is already linked to a participant");
        }
      }
      const updated_at = iso(ctx.now + ++seq * MIN);
      refs.set(id, { authority, mrn, updated_at });
      return { participant_id: id, assigning_authority: authority, updated_at };
    },

    async deletePatientRef(id) {
      await find(id);
      if (!refs.delete(id)) throw problem(404, "not-found", "Not found");
    },

    async searchPatientRef(orgId, req) {
      if (orgId !== ctx.orgId) throw problem(404, "not-found", "Not found");
      await all();
      const authority = req.assigning_authority.trim().toUpperCase();
      if (!authority) throw invalid("assigning_authority", "must not be empty");
      if (!normalise(req.mrn)) throw invalid("mrn", "must not be empty");
      for (const [id, r] of refs) {
        if (r.authority === authority && normalise(r.mrn) === normalise(req.mrn)) return { participant_id: id };
      }
      return { participant_id: null };
    },

    async listParticipantDisclosures(id, query = {}) {
      await find(id);
      const rows = disclosures.get(id) ?? [];
      const { slice, next_cursor, total } = ctx.page(rows, (d) => [d.created_at, d.export_id], query.cursor, query.limit);
      return { items: slice, next_cursor, total };
    },
  };
}
