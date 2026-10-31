/**
 * Fixture-backed implementation of the typed API for UI development and tests without a backend.
 * All data is synthetic and deterministic. Never contains real child data.
 */
import {
  ApiError,
  PROBLEM_BASE,
  type AuthApi,
  type ClipScore,
  type ClipStatus,
  type CurvApi,
  type Device,
  type EnrolCodeRequest,
  type Metrics,
  type MetricRow,
  type ReviewItem,
  type ReviewReason,
  type Role,
  REVIEW_STATUSES,
  ROLES,
  type SessionDetail,
  type SessionListQuery,
  type SessionSummary,
  type TokenPair,
} from "./types";
import { createMockClinical } from "./mock/clinical";
import { MOCK_TRACKING_CYCLE, mockTrackingQuality } from "./mock/tracking";

/** Real orgs are UUIDs (CON-20); the mock uses a fixed one so URLs look like production. */
export const MOCK_ORG_ID = "5f0e1d2c-3b4a-4968-8776-655443322110";
export const MOCK_PASSWORD = "demo";
export const MOCK_FIXED_NOW = Date.parse("2026-11-01T12:00:00Z");
const DEFAULT_LIMIT = 20;

const FLAGS = ["green", "green", "green", "yellow", "red", "unreliable"] as const;
// Values of the backend's GazeQualityVerdict.
const QUALITY = ["optimal", "optimal", "acceptable", "degraded", "rejected"] as const;
// Catalog protocol ids (CON-16).
const BATTERIES = ["express_30s", "comprehensive_60s"] as const;
// The backend rejects `simulated` always (SIM-5), so the mock only emits the two accepted sources.
const SOURCES = ["truedepth", "truedepth", "truedepth", "synthetic"] as const;
const CLIPS = [
  "stim_01_social_geo_10s", "stim_02_gaze_cue_8s", "stim_03_bio_motion_10s", "stim_04_face_eye_8s",
  "stim_05_face_mouth_8s", "stim_06_joint_attn_10s", "stim_07_orienting_6s", "stim_08_affect_10s",
];
const METRICS: { metric: string; segment_id: string; target_aoi: string | null; direction: MetricRow["direction"] }[] = [
  { metric: "social_dwell_ratio", segment_id: "split", target_aoi: "social", direction: "higherBetter" },
  { metric: "eye_dwell_ratio", segment_id: "faces", target_aoi: "eyes", direction: "higherBetter" },
  { metric: "mouth_dwell_ratio", segment_id: "faces", target_aoi: "mouth", direction: "band" },
  { metric: "orienting_latency_ms", segment_id: "cue", target_aoi: null, direction: "lowerBetter" },
];
const STATUS_RANK: Record<ClipStatus, number> = { onTarget: 0, monitor: 1, atypical: 2 };

function uuid(r: () => number): string {
  const h = (n: number) => Array.from({ length: n }, () => Math.floor(r() * 16).toString(16)).join("");
  return `${h(8)}-${h(4)}-4${h(3)}-8${h(3)}-${h(12)}`;
}

/** Opaque keyset cursor over `(started_at, id)`, base64url like the backend's (API list conventions). */
function encodeCursor(startedAt: string, id: string): string {
  return btoa(`${startedAt}|${id}`).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
}
function decodeCursor(cursor: string): { startedAt: string; id: string } | null {
  try {
    const raw = atob(cursor.replace(/-/g, "+").replace(/_/g, "/"));
    const i = raw.indexOf("|");
    return i > 0 ? { startedAt: raw.slice(0, i), id: raw.slice(i + 1) } : null;
  } catch {
    return null;
  }
}

// Small deterministic PRNG so fixtures are stable across runs.
function rng(seed: number): () => number {
  let s = (Math.imul(seed, 2654435761) ^ 0x9e3779b9) >>> 0;
  return () => {
    // mulberry32
    s = (s + 0x6d2b79f5) >>> 0;
    let t = s;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

const DEVICES = Array.from({ length: 4 }, (_, k) => uuid(rng(9000 + k)));

function participant(ids: string[], k: number): { participant_id: string; participant_token: string } {
  return { participant_id: ids[k]!, participant_token: `child-${String(k + 1).padStart(3, "0")}` };
}

function buildFixtures(): { details: SessionDetail[]; clips: Map<string, ClipScore[]> } {
  const details: SessionDetail[] = [];
  const clips = new Map<string, ClipScore[]>();
  const participants = Array.from({ length: 18 }, (_, k) => uuid(rng(7000 + k)));
  for (let i = 0; i < 64; i++) {
    const r = rng(1000 + i);
    const flag = FLAGS[Math.floor(r() * FLAGS.length)]!;
    const quality = QUALITY[Math.floor(r() * QUALITY.length)]!;
    const id = `SESS-MOCK-${String(i + 1).padStart(4, "0")}`;
    const started = new Date(MOCK_FIXED_NOW - i * 7.3 * 3600_000).toISOString();
    const unreliable = flag === "unreliable";
    // Every 3rd session is schema 1.7 with a tracking record; the rest are 1.6 (null tracking, SCH-4).
    const trackingVerdict = i % 3 === 0 ? MOCK_TRACKING_CYCLE[(i / 3) % MOCK_TRACKING_CYCLE.length]! : null;
    const duration = Math.round(150 + r() * 200);
    const tracking = trackingVerdict ? mockTrackingQuality(trackingVerdict, duration) : null;
    details.push({
      id,
      org_id: MOCK_ORG_ID,
      ...participant(participants, Math.floor(r() * 18)),
      started_at: started,
      duration_sec: duration,
      battery: BATTERIES[Math.floor(r() * BATTERIES.length)]!,
      capture_source: SOURCES[Math.floor(r() * SOURCES.length)]!,
      quality_verdict: quality,
      attention_flag: flag,
      composite_score: unreliable ? null : Math.round(r() * 1000) / 10,
      composite_confidence: unreliable ? 0.3 : Math.round((0.55 + r() * 0.45) * 100) / 100,
      bundle_version: i % 5 === 0 ? "2026.10.1" : "2026.11.0",
      device_id: DEVICES[i % DEVICES.length]!,
      reviewed: r() > 0.6,
      age_pack: ["2-3y", "3-4y", "4-5y"][i % 3]!,
      lang: "en",
      schema_version: tracking ? "1.7" : "1.6",
      valid_frame_ratio: Math.round((0.5 + r() * 0.5) * 100) / 100,
      // An "unusable" 1.7 session never calibrated, so it has no distance (shown as "Not measured").
      calibrated_distance_cm: trackingVerdict === "unusable" ? null : Math.round(40 + r() * 20),
      social_preference: Math.round(r() * 100) / 100,
      eye_dwell: Math.round(r() * 100) / 100,
      mouth_dwell: Math.round(r() * 60) / 100,
      first_eye_latency_ms: Math.round(300 + r() * 900),
      affect: { engagement: Math.round(r() * 100) / 100 },
      report_pdf_available: !unreliable,
      tracking_verdict: trackingVerdict,
      tracking_accuracy_deg: tracking?.estimated_accuracy_deg ?? null,
      tracking_quality: tracking,
    });
    clips.set(
      id,
      CLIPS.slice(0, 4 + (i % 2) * 4).map((clip, order) => {
        // Every 5th clip has no scored rows (status null); every 3rd has a single row; the rest have several.
        const rowCount = (order + i) % 5 === 4 ? 0 : (order + i) % 3 === 0 ? 1 : 2 + (order % 3);
        const rows: MetricRow[] = METRICS.slice(0, rowCount).map((m) => {
          const expected = Math.round(r() * 80) / 100;
          const recorded = Math.round(r() * 80) / 100;
          const deviation = Math.round((recorded - expected) * 100) / 100;
          const abs = Math.abs(deviation);
          return {
            ...m,
            expected,
            acceptable_low: Math.max(0, Math.round((expected - 0.15) * 100) / 100),
            acceptable_high: m.direction === "band" ? Math.round((expected + 0.15) * 100) / 100 : null,
            recorded,
            deviation,
            status: abs < 0.15 ? "onTarget" : abs < 0.35 ? "monitor" : "atypical",
            score: Math.round(r() * 100) / 100,
            imputed: r() > 0.9,
            source: "literature",
          } satisfies MetricRow;
        });
        // The backend fills the flat columns from the worst-status row (CON-19); the mock mirrors that shape.
        const worst = rows.reduce<MetricRow | undefined>(
          (w, row) =>
            !w ||
            STATUS_RANK[row.status] > STATUS_RANK[w.status] ||
            (row.status === w.status && Math.abs(row.deviation ?? 0) > Math.abs(w.deviation ?? 0))
              ? row
              : w,
          undefined,
        );
        return {
          clip_id: clip,
          order,
          status: worst?.status ?? null,
          score: worst ? Math.round(r() * 100) / 100 : null,
          weight: 1 / 4,
          dwell_social: Math.round(r() * 100) / 100,
          dwell_geometric: Math.round(r() * 100) / 100,
          dwell_eye: Math.round(r() * 100) / 100,
          dwell_mouth: Math.round(r() * 100) / 100,
          orienting_latency_ms: Math.round(200 + r() * 800),
          gaze_cue_accuracy: Math.round(r() * 100) / 100,
          fixation_count: 5 + Math.floor(r() * 20),
          mean_fixation_ms: Math.round(180 + r() * 300),
          metric: worst?.metric ?? null,
          expected: worst?.expected ?? null,
          recorded: worst?.recorded ?? null,
          deviation: worst?.deviation ?? null,
          baseline_ref: worst ? "provisional-literature-seeded" : null,
          rows,
        } satisfies ClipScore;
      }),
    );
  }
  return { details, clips };
}

function toSummary(d: SessionDetail): SessionSummary {
  return {
    id: d.id,
    participant_id: d.participant_id,
    participant_token: d.participant_token,
    started_at: d.started_at,
    duration_sec: d.duration_sec,
    battery: d.battery,
    capture_source: d.capture_source,
    quality_verdict: d.quality_verdict,
    attention_flag: d.attention_flag,
    composite_score: d.composite_score,
    composite_confidence: d.composite_confidence,
    bundle_version: d.bundle_version,
    device_id: d.device_id,
    reviewed: d.reviewed,
    tracking_verdict: d.tracking_verdict,
    tracking_accuracy_deg: d.tracking_accuracy_deg,
  };
}

function invalidCursor(): ApiError {
  return new ApiError(400, {
    type: `${PROBLEM_BASE}validation-failed`,
    title: "Validation failed",
    status: 400,
    invalid_params: [{ name: "cursor", reason: "invalid" }],
  });
}

function notFound(detail: string): ApiError {
  return new ApiError(404, { type: `${PROBLEM_BASE}not-found`, title: "Not found", status: 404, detail });
}

function unauthorized(detail: string): ApiError {
  return new ApiError(401, { type: `${PROBLEM_BASE}unauthorized`, title: "Unauthorized", status: 401, detail });
}

function tally(values: string[]): Record<string, number> {
  const out: Record<string, number> = {};
  for (const v of values) out[v] = (out[v] ?? 0) + 1;
  return out;
}

const MOCK_USER_ID = "0b7c9e2a-5d1f-4c3e-9a8b-1f2e3d4c5b6a";

/** The email's local part picks the role (`viewer@...` signs in as a viewer) so role-gated UI can be tried. */
function roleFor(email: string): Role {
  const local = email.split("@")[0]?.toLowerCase() ?? "";
  return (ROLES as readonly string[]).includes(local) ? (local as Role) : "admin";
}

/** Tokens carry the email so refresh can restore the same user. Mock only: never do this with real tokens. */
function pair(email: string): TokenPair {
  const role = roleFor(email);
  return {
    access_token: `mock-access.${btoa(email)}`,
    refresh_token: `mock-refresh.${btoa(email)}`,
    expires_in: 900,
    user: { id: MOCK_USER_ID, email, role, org_id: role === "superadmin" ? null : MOCK_ORG_ID },
  };
}

function validation(name: string, reason: string): ApiError {
  return new ApiError(400, {
    type: `${PROBLEM_BASE}validation-failed`,
    title: "Validation failed",
    status: 400,
    invalid_params: [{ name, reason }],
  });
}

/** One review item per session at most, picked by the first matching rule (the backend may add several). */
function reviewReasonFor(d: SessionDetail, i: number): ReviewReason | null {
  if (i % 11 === 5) return "manual";
  if (d.tracking_verdict === "poor" || d.tracking_verdict === "unusable") return "low_tracking_quality";
  if (d.attention_flag === "red") return "flag_red";
  if (d.quality_verdict === "rejected" || d.quality_verdict === "degraded") return "low_quality";
  if (d.capture_source === "synthetic") return "norm_candidate";
  if (i % 7 === 0) return "random_audit";
  return null;
}

function buildReviewItems(details: SessionDetail[]): ReviewItem[] {
  const items: ReviewItem[] = [];
  details.forEach((d, i) => {
    const reason = reviewReasonFor(d, i);
    if (!reason) return;
    const closed = d.reviewed === true;
    const decidedAt = new Date(Date.parse(d.started_at) + 86_400_000).toISOString();
    items.push({
      id: uuid(rng(5000 + i)),
      org_id: MOCK_ORG_ID,
      session_id: d.id,
      reason,
      status: closed ? (i % 2 ? "done" : "dismissed") : i % 9 === 1 ? "in_review" : "open",
      created_at: new Date(Date.parse(d.started_at) + 10 * 60_000).toISOString(),
      assigned_to: i % 9 === 1 ? MOCK_USER_ID : null,
      decision: closed ? { action: i % 2 ? "confirm" : "dismiss" } : null,
      decided_by: closed ? MOCK_USER_ID : null,
      decided_at: closed ? decidedAt : null,
    });
  });
  return items;
}

function buildDevices(details: SessionDetail[]): Device[] {
  const lastSeen = (id: string) => details.find((d) => d.device_id === id)?.started_at ?? null;
  const devices: Device[] = DEVICES.map((id, k) => ({
    id,
    org_id: MOCK_ORG_ID,
    platform: "ios",
    app_version: k === 3 ? "1.3.2" : "1.4.0",
    enrolled_at: new Date(MOCK_FIXED_NOW - (60 - k * 7) * 86_400_000).toISOString(),
    label: `iPad room ${k + 1}`,
    revoked_at: null,
    last_seen_at: lastSeen(id),
    config_bundle_version: k === 3 ? "2026.10.1" : "2026.11.0",
  }));
  devices.push({
    id: uuid(rng(9100)),
    org_id: MOCK_ORG_ID,
    platform: "ios",
    app_version: "1.2.0",
    enrolled_at: new Date(MOCK_FIXED_NOW - 120 * 86_400_000).toISOString(),
    label: "Lost iPad",
    revoked_at: new Date(MOCK_FIXED_NOW - 30 * 86_400_000).toISOString(),
    last_seen_at: null,
    config_bundle_version: null,
  });
  return devices.sort((a, b) => b.enrolled_at.localeCompare(a.enrolled_at));
}

const ENROL_ALPHABET = "ABCDEFGHJKMNPQRSTVWXYZ23456789";

function enrolCode(): string {
  const pick = () => ENROL_ALPHABET[Math.floor(Math.random() * ENROL_ALPHABET.length)]!;
  const part = () => Array.from({ length: 4 }, pick).join("");
  return `${part()}-${part()}`;
}

function page<T>(rows: T[], key: (row: T) => [string, string], cursor: string | undefined, limit = DEFAULT_LIMIT) {
  const size = Math.min(Math.max(limit, 1), 100);
  let start = 0;
  if (cursor) {
    const after = decodeCursor(cursor);
    if (!after) throw invalidCursor();
    start = rows.findIndex((r) => key(r)[0] === after.startedAt && key(r)[1] === after.id) + 1;
    if (start === 0) throw invalidCursor();
  }
  const slice = rows.slice(start, start + size);
  const last = slice[slice.length - 1];
  const more = start + size < rows.length;
  return { slice, next_cursor: more && last ? encodeCursor(...key(last)) : null, total: rows.length };
}

export function createMockClient(): CurvApi & AuthApi {
  const { details, clips } = buildFixtures();
  const reviews = buildReviewItems(details);
  const devices = buildDevices(details);
  const labels: { session_id: string; key: string; value: unknown; clip_id?: string | null }[] = [];
  const ensureOrg = (orgId: string) => {
    if (orgId !== MOCK_ORG_ID) throw notFound("Organisation not found");
  };

  return {
    ...createMockClinical({ details, orgId: MOCK_ORG_ID, userId: MOCK_USER_ID, now: MOCK_FIXED_NOW, ensureOrg, page }),
    async login(email, password) {
      if (!email || password !== MOCK_PASSWORD) {
        throw unauthorized("Invalid email or password");
      }
      return pair(email);
    },
    async refresh(refreshToken) {
      const prefix = "mock-refresh.";
      if (!refreshToken.startsWith(prefix)) throw unauthorized("Invalid refresh token");
      let email: string;
      try {
        email = atob(refreshToken.slice(prefix.length));
      } catch {
        throw unauthorized("Invalid refresh token");
      }
      return pair(email);
    },
    async logout() {},
    async getOrg(orgId) {
      if (orgId !== MOCK_ORG_ID) throw notFound("Organisation not found");
      return { id: MOCK_ORG_ID, name: "Demo Clinic (synthetic)", kind: "clinic", retention_months: 24 };
    },
    async getMetrics(orgId) {
      if (orgId !== MOCK_ORG_ID) throw notFound("Organisation not found");
      const within = (days: number) =>
        details.filter((d) => Date.parse(d.started_at) >= MOCK_FIXED_NOW - days * 86_400_000);
      const last30 = within(30);
      const bundles = tally(details.map((d) => d.bundle_version ?? "unknown"));
      const metrics: Metrics = {
        sessions_7d: within(7).length,
        sessions_30d: last30.length,
        flag_distribution: tally(last30.map((d) => d.attention_flag)),
        quality_distribution: tally(last30.map((d) => d.quality_verdict)),
        review_queue_size: reviews.filter((r) => r.status === "open" || r.status === "in_review").length,
        devices_active: devices.filter((d) => !d.revoked_at).length,
        bundle_adoption: Object.entries(bundles).map(([bundle_version, devices]) => ({ bundle_version, devices })),
      };
      return metrics;
    },
    async listSessions(orgId, query: SessionListQuery = {}) {
      if (orgId !== MOCK_ORG_ID) throw notFound("Organisation not found");
      const from = query.from ? Date.parse(query.from) : undefined;
      // `to` is an inclusive calendar day when given as YYYY-MM-DD.
      const to = query.to ? Date.parse(query.to) + (query.to.length <= 10 ? 86_400_000 - 1 : 0) : undefined;
      const filtered = details.filter((d) => {
        const t = Date.parse(d.started_at);
        return (
          (!query.flag || d.attention_flag === query.flag) &&
          (!query.quality || d.quality_verdict === query.quality) &&
          (!query.tracking || d.tracking_verdict === query.tracking) &&
          (!query.bundle_version || d.bundle_version === query.bundle_version) &&
          (query.reviewed === undefined || Boolean(d.reviewed) === query.reviewed) &&
          (from === undefined || t >= from) &&
          (to === undefined || t <= to)
        );
      });
      // `details` is ordered newest first, which is the backend's `(started_at, id)` descending order.
      const { slice, next_cursor, total } = page(filtered, (d) => [d.started_at, d.id], query.cursor, query.limit);
      return { items: slice.map(toSummary), next_cursor, total };
    },
    async getSession(sessionId) {
      const found = details.find((d) => d.id === sessionId);
      if (!found) throw notFound("Session not found");
      return found;
    },
    async getSessionClips(sessionId) {
      const found = clips.get(sessionId);
      if (!found) throw notFound("Session not found");
      return found;
    },
    async listReview(orgId, query = {}) {
      ensureOrg(orgId);
      const rows = reviews
        .filter((r) => (!query.status || r.status === query.status) && (!query.reason || r.reason === query.reason))
        .sort((a, b) => b.created_at.localeCompare(a.created_at) || b.id.localeCompare(a.id));
      const { slice, next_cursor, total } = page(rows, (r) => [r.created_at, r.id], query.cursor, query.limit);
      return { items: slice.map((r) => ({ ...r })), next_cursor, total };
    },
    async updateReview(reviewId, patch) {
      const item = reviews.find((r) => r.id === reviewId);
      if (!item) throw notFound("Review item not found");
      if (patch.status != null && !(REVIEW_STATUSES as readonly string[]).includes(patch.status)) {
        throw validation("status", "must be open, in_review, done or dismissed");
      }
      if (patch.assigned_to != null && patch.assigned_to !== MOCK_USER_ID) {
        throw validation("assigned_to", "must be a user id of this org");
      }
      if (patch.status != null) {
        item.status = patch.status;
        const closed = patch.status === "done" || patch.status === "dismissed";
        item.decided_by = closed ? MOCK_USER_ID : null;
        item.decided_at = closed ? new Date().toISOString() : null;
        const session = details.find((d) => d.id === item.session_id);
        if (session && closed) session.reviewed = true;
      }
      if (patch.assigned_to != null) item.assigned_to = patch.assigned_to;
      if (patch.decision !== undefined && patch.decision !== null) item.decision = patch.decision;
      for (const l of patch.labels ?? []) labels.push({ session_id: item.session_id, ...l });
      return { ...item };
    },
    async listDevices(orgId, query = {}) {
      ensureOrg(orgId);
      const { slice, next_cursor } = page(devices, (d) => [d.enrolled_at, d.id], query.cursor, query.limit);
      // The backend does not count devices (total: null).
      return { items: slice.map((d) => ({ ...d })), next_cursor, total: null };
    },
    async revokeDevice(deviceId) {
      const device = devices.find((d) => d.id === deviceId);
      if (!device) throw notFound("Device not found");
      device.revoked_at ??= new Date().toISOString();
      return { ...device };
    },
    async createEnrolCode(orgId, request: EnrolCodeRequest = {}) {
      ensureOrg(orgId);
      const maxUses = request.max_uses ?? 1;
      const days = request.expires_in_days ?? 7;
      if (!Number.isInteger(maxUses) || maxUses < 1 || maxUses > 1000) throw validation("max_uses", "must be 1 to 1000");
      if (!Number.isInteger(days) || days < 1 || days > 90) throw validation("expires_in_days", "must be 1 to 90");
      if ((request.label?.trim().length ?? 0) > 200) throw validation("label", "must be at most 200 characters");
      return {
        id: uuid(Math.random),
        code: enrolCode(),
        max_uses: maxUses,
        expires_at: new Date(Date.now() + days * 86_400_000).toISOString(),
      };
    },
  };
}
