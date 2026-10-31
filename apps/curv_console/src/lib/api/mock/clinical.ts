/**
 * Fixture-backed implementation of the clinician workspace API for mock mode (WP-C.13). Synthetic data only.
 *
 * The `content_hash` here is a FAKE deterministic digest (FNV-1a repeated to 64 hex chars) that exists only so the
 * mock can honour the "echo the displayed hash" rule (409 content-changed). It is not SHA-256 and nothing in the
 * console computes a real hash.
 */
import { ApiError, PROBLEM_BASE, type Org, type SessionDetail } from "../types";
import {
  SIGNATURE_MEANING,
  type ClinicalApi,
  type ClinicalReview,
  type ClinicalSettingsPatch,
  type ClinicalState,
  type CosignPolicy,
  type WorklistItem,
  type WorklistScope,
} from "../clinical";

export interface MockClinicalContext {
  details: SessionDetail[];
  orgId: string;
  /** The signed-in mock user (the mock backend has one caller identity). */
  userId: string;
  now: number;
  ensureOrg: (orgId: string) => void;
  page: <T>(
    rows: T[],
    key: (row: T) => [string, string],
    cursor: string | undefined,
    limit?: number,
  ) => { slice: T[]; next_cursor: string | null; total: number };
}

const OTHER_USER = "6d5c4b3a-2f1e-4d0c-9b8a-7f6e5d4c3b2a";
const PASSWORD = "demo";

function problem(status: number, slug: string, title: string, detail?: string): ApiError {
  return new ApiError(status, { type: `${PROBLEM_BASE}${slug}`, title, status, detail });
}

function fakeHash(...parts: (string | number | null | undefined)[]): string {
  const text = parts.map((p) => String(p ?? "")).join("\u0001");
  let out = "";
  for (let seed = 0; seed < 8; seed++) {
    let h = (0x811c9dc5 ^ (seed * 0x9e3779b1)) >>> 0;
    for (let i = 0; i < text.length; i++) h = Math.imul(h ^ text.charCodeAt(i), 0x01000193) >>> 0;
    out += h.toString(16).padStart(8, "0");
  }
  return out;
}

export function createMockClinical(ctx: MockClinicalContext): ClinicalApi {
  const { details, orgId, userId } = ctx;
  const reviews = new Map<string, ClinicalReview>(); // keyed by session id
  const settings: { cosign: CosignPolicy; scope: WorklistScope } = { cosign: "none", scope: "all" };
  const profile = {
    id: userId,
    email: "demo@example.org",
    role: "clinician" as const,
    org_id: orgId,
    display_name: "Dr. Demo Clinician" as string | null,
    professional_registration: "NMC 00000" as string | null,
  };
  let seq = 0;
  const newId = () => `c0ffee00-0000-4000-8000-${String(++seq).padStart(12, "0")}`;

  function rehash(r: ClinicalReview): void {
    r.content_hash = fakeHash(r.session_id, r.current.version_no, r.current.disposition, r.current.impression, r.current.recommendations);
  }

  function coSignRequired(session: SessionDetail): boolean {
    return settings.cosign === "always" || (settings.cosign === "red_flag" && session.attention_flag === "red");
  }

  function create(session: SessionDetail, state: ClinicalState, assignedTo: string | null): ClinicalReview {
    const at = new Date(ctx.now).toISOString();
    const r: ClinicalReview = {
      id: newId(),
      org_id: orgId,
      session_id: session.id,
      state,
      assigned_to: assignedTo,
      cosign_required: coSignRequired(session),
      row_version: 1,
      current: { version_no: 1, status: "draft", created_at: at, created_by: assignedTo, disposition: null, impression: null, recommendations: null, amend_reason: null, signed_at: null },
      content_hash: "",
      signatures: [],
      created_at: at,
      updated_at: at,
    };
    rehash(r);
    reviews.set(session.id, r);
    return r;
  }

  function signature(r: ClinicalReview, role: "sign" | "cosign", signerId: string, name: string, at: string) {
    return {
      id: newId(), version_no: r.current.version_no, role, signer_id: signerId, signer_display_name: name,
      signer_registration: "NMC 00000", meaning: SIGNATURE_MEANING, content_hash: r.content_hash, signed_at: at,
    };
  }

  // Seeds: one awaiting a cosign from the demo user (signed by someone else), one final, one draft in progress.
  const seed = (i: number) => details[i];
  const s2 = seed(2);
  if (s2) {
    const r = create(s2, "in_progress", OTHER_USER);
    r.cosign_required = true;
    r.current = { ...r.current, disposition: "monitor", impression: "Synthetic seed impression.", recommendations: "Re-screen in 3 months." };
    rehash(r);
    const at = new Date(ctx.now - 3_600_000).toISOString();
    r.signatures = [signature(r, "sign", OTHER_USER, "Dr. Other Clinician", at)];
    r.current = { ...r.current, status: "signed", signed_at: at };
    r.state = "awaiting_cosign";
  }
  const s3 = seed(3);
  if (s3) {
    const r = create(s3, "in_progress", userId);
    r.current = { ...r.current, disposition: "no_concern_noted", impression: "Synthetic seed impression.", recommendations: null };
    rehash(r);
    const at = new Date(ctx.now - 7_200_000).toISOString();
    r.signatures = [signature(r, "sign", userId, "Dr. Demo Clinician", at)];
    r.current = { ...r.current, status: "signed", signed_at: at };
    r.state = "final";
  }
  const s4 = seed(4);
  if (s4) {
    const r = create(s4, "in_progress", userId);
    r.current = { ...r.current, disposition: null, impression: "Draft in progress (synthetic).", recommendations: null };
    r.row_version = 2;
    rehash(r);
  }

  const byId = (id: string): ClinicalReview => {
    const found = [...reviews.values()].find((r) => r.id === id);
    if (!found) throw problem(404, "not-found", "Not found", "Clinical review not found");
    return found;
  };
  const snapshot = (r: ClinicalReview): ClinicalReview => structuredClone(r);
  const touch = (r: ClinicalReview) => {
    r.row_version += 1;
    r.updated_at = new Date().toISOString();
    rehash(r);
  };

  function stepUp(r: ClinicalReview, body: { password: string; content_hash: string; meaning_ack: boolean }): void {
    if (!body.meaning_ack) throw problem(400, "validation-failed", "Validation failed", "meaning_ack must be true");
    if (!profile.display_name) {
      throw problem(409, "signer-profile-incomplete", "Signer profile incomplete", "Set your display name first");
    }
    if (body.password !== PASSWORD) {
      throw problem(401, "reauthentication-failed", "Re-authentication failed");
    }
    if (body.content_hash !== r.content_hash) {
      throw problem(409, "content-changed", "Content changed", "The review changed after you opened it");
    }
  }

  return {
    async getWorklist(org, query = {}) {
      ctx.ensureOrg(org);
      const from = query.from ? Date.parse(query.from) : undefined;
      const to = query.to ? Date.parse(query.to) + (query.to.length <= 10 ? 86_400_000 - 1 : 0) : undefined;
      const rows: WorklistItem[] = details
        .filter((d) => d.org_id === org)
        .flatMap((d) => {
          const r = reviews.get(d.id);
          if (r?.state === "final") return [];
          if (settings.scope === "flagged" && d.attention_flag === "green") return [];
          const t = Date.parse(d.started_at);
          if (from !== undefined && t < from) return [];
          if (to !== undefined && t > to) return [];
          if (query.flag && d.attention_flag !== query.flag) return [];
          if (query.state && (r?.state ?? null) !== query.state) return [];
          if (query.assigned === "me" && r?.assigned_to !== userId) return [];
          if (query.assigned === "unassigned" && r?.assigned_to) return [];
          return [
            {
              id: d.id, participant_id: d.participant_id, participant_token: d.participant_token,
              started_at: d.started_at, duration_sec: d.duration_sec, battery: d.battery,
              capture_source: d.capture_source, quality_verdict: d.quality_verdict,
              attention_flag: d.attention_flag, composite_score: d.composite_score,
              composite_confidence: d.composite_confidence, bundle_version: d.bundle_version,
              device_id: d.device_id, reviewed: d.reviewed,
              age_hours: Math.round(((ctx.now - t) / 3_600_000) * 10) / 10,
              assigned_to: r?.assigned_to ?? null,
              clinical_state: r?.state ?? null,
              cosign_required: r?.cosign_required ?? coSignRequired(d),
              has_patient_ref: d.id.endsWith("7"),
              review_id: r?.id ?? null,
            } satisfies WorklistItem,
          ];
        });
      const { slice, next_cursor, total } = ctx.page(rows, (w) => [w.started_at, w.id], query.cursor, query.limit);
      return { items: slice, next_cursor, total };
    },
    async getSessionClinicalReview(sessionId) {
      const r = reviews.get(sessionId);
      if (!r) {
        if (!details.some((d) => d.id === sessionId)) throw problem(404, "not-found", "Not found");
        return null;
      }
      return snapshot(r);
    },
    async getClinicalReview(id) {
      return snapshot(byId(id));
    },
    async claimClinicalReview(sessionId, request) {
      const session = details.find((d) => d.id === sessionId);
      if (!session) throw problem(404, "not-found", "Not found", "Session not found");
      const existing = reviews.get(sessionId);
      if (existing) return snapshot(existing);
      return snapshot(create(session, "open", request?.assigned_to ?? userId));
    },
    async saveClinicalDraft(id, rowVersion, patch) {
      const r = byId(id);
      if (rowVersion !== r.row_version) {
        throw problem(412, "stale-version", "Stale version", "The review was changed by someone else");
      }
      if (r.state !== "open" && r.state !== "in_progress" && r.state !== "amending") {
        throw problem(409, "conflict", "Conflict", "A signed review cannot be edited; amend it first");
      }
      if ((patch.impression?.length ?? 0) > 8000 || (patch.recommendations?.length ?? 0) > 8000) {
        throw problem(400, "validation-failed", "Validation failed", "impression and recommendations are at most 8000 characters");
      }
      if (patch.disposition !== undefined) r.current.disposition = patch.disposition;
      if (patch.impression !== undefined) r.current.impression = patch.impression;
      if (patch.recommendations !== undefined) r.current.recommendations = patch.recommendations;
      if (patch.assigned_to !== undefined) r.assigned_to = patch.assigned_to;
      if (r.state === "open") r.state = "in_progress";
      touch(r);
      return snapshot(r);
    },
    async signClinicalReview(id, body) {
      const r = byId(id);
      if (r.state !== "open" && r.state !== "in_progress" && r.state !== "amending") {
        throw problem(409, "conflict", "Conflict", "This review is not awaiting a signature");
      }
      if (!r.current.disposition) throw problem(400, "validation-failed", "Validation failed", "disposition is required to sign");
      stepUp(r, body);
      const at = new Date().toISOString();
      r.signatures.push(signature(r, "sign", userId, profile.display_name ?? "", at));
      r.current.status = "signed";
      r.current.signed_at = at;
      r.state = r.cosign_required ? "awaiting_cosign" : "final";
      touch(r);
      return snapshot(r);
    },
    async cosignClinicalReview(id, body) {
      const r = byId(id);
      if (r.state !== "awaiting_cosign") throw problem(409, "conflict", "Conflict", "This review is not awaiting a cosignature");
      const first = r.signatures.find((s) => s.role === "sign" && s.version_no === r.current.version_no);
      if (first?.signer_id === userId) {
        throw problem(409, "cosigner-must-differ", "Cosigner must differ", "A different clinician must cosign");
      }
      stepUp(r, body);
      r.signatures.push(signature(r, "cosign", userId, profile.display_name ?? "", new Date().toISOString()));
      r.state = "final";
      touch(r);
      return snapshot(r);
    },
    async amendClinicalReview(id, { reason }) {
      const r = byId(id);
      if (r.state !== "final" && r.state !== "awaiting_cosign") {
        throw problem(409, "conflict", "Conflict", "Only a signed review can be amended");
      }
      const n = reason.trim().length;
      if (n < 10 || n > 500) throw problem(400, "validation-failed", "Validation failed", "reason must be 10 to 500 characters");
      r.current = {
        ...r.current, version_no: r.current.version_no + 1, status: "draft", signed_at: null,
        created_at: new Date().toISOString(), created_by: userId, amend_reason: reason.trim(),
      };
      r.state = "amending";
      touch(r);
      return snapshot(r);
    },
    async getMyProfile() {
      return { ...profile };
    },
    async updateMyProfile(patch) {
      if (patch.display_name !== undefined) profile.display_name = patch.display_name?.trim() || null;
      if (patch.professional_registration !== undefined) {
        profile.professional_registration = patch.professional_registration?.trim() || null;
      }
      return { ...profile };
    },
    async updateOrgClinicalSettings(org, patch: ClinicalSettingsPatch): Promise<Org> {
      ctx.ensureOrg(org);
      if (patch.clinical?.cosign_policy) settings.cosign = patch.clinical.cosign_policy;
      if (patch.clinical?.worklist_scope) settings.scope = patch.clinical.worklist_scope;
      return { id: orgId, name: "Demo Clinic (synthetic)", kind: "clinic", retention_months: 24 };
    },
  };
}
