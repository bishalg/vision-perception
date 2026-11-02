/** Fixture-backed administration API for mock mode (WP-C.3). Synthetic data only; no real names or e-mail addresses. */
import {
  ApiError,
  PROBLEM_BASE,
  type Device,
  type SessionDetail,
} from "../types";
import {
  MAX_TOTP_RESET_REASON_CHARS,
  MIN_TOTP_RESET_REASON_CHARS,
  type AdminApi,
  type AuditEntry,
  type AuditQuery,
  type CreateUserRequest,
  type OrgUser,
  type Participant,
  type UpdateUserRequest,
} from "../admin";
import { registerMockRecordStore } from "./emr";

/** The mock caller's step-up password; equals `MOCK_PASSWORD` in `mock.ts` (checked by tests/totp-reset.test.tsx). */
export const MOCK_STEP_UP_PASSWORD = "demo";

const RANK: Record<OrgUser["role"], number> = {
  superadmin: 3, owner: 2, admin: 1, clinician: 0, researcher: 0, reviewer: 0, viewer: 0,
};

export interface MockAdminContext {
  details: SessionDetail[];
  devices: Device[];
  orgId: string;
  /** The signed-in mock user; listed among the users so the "cannot edit yourself" rule can be tried. */
  userId: string;
  now: number;
  ensureOrg: (orgId: string) => void;
  page: <T>(
    rows: T[],
    key: (row: T) => [string, string],
    cursor: string | undefined,
    limit?: number,
  ) => { slice: T[]; next_cursor: string | null; total: number };
  uuid: (seed: number) => string;
  /** Participant id -> withdrawal time, written by the privacy mock when an erasure is filed (WP-B.9). */
  withdrawn?: Map<string, string>;
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

const DAY = 86_400_000;

export function createMockAdmin(ctx: MockAdminContext): AdminApi {
  const { orgId, userId, now } = ctx;
  const iso = (ms: number) => new Date(ms).toISOString();

  // Participants: derived from the session fixtures (same ids and tokens), newest first by first session.
  const byParticipant = new Map<string, SessionDetail[]>();
  for (const d of ctx.details) byParticipant.set(d.participant_id, [...(byParticipant.get(d.participant_id) ?? []), d]);
  const participants: (Participant & { created_at: string })[] = [...byParticipant.entries()]
    .map(([id, sessions], k) => {
      const first = sessions[sessions.length - 1]!;
      const last = sessions[0]!;
      return {
        id,
        org_id: orgId,
        participant_token: first.participant_token,
        age_bracket_at_enrol: first.age_pack ?? null,
        language: k % 5 === 4 ? "ne" : "en",
        session_count: sessions.length,
        last_session_at: last.started_at,
        withdrawn_at: k === 7 ? iso(now - 3 * DAY) : null,
        has_patient_ref: k % 3 === 0,
        created_at: first.started_at,
      };
    })
    .sort((a, b) => b.created_at.localeCompare(a.created_at) || b.id.localeCompare(a.id));

  const person = (n: number, role: OrgUser["role"], over: Partial<OrgUser> = {}): OrgUser => ({
    id: ctx.uuid(8000 + n),
    email: `${role}${n}@demo.example.org`,
    role,
    org_id: orgId,
    status: "active",
    last_login_at: iso(now - n * 5 * 3_600_000),
    display_name: null,
    professional_registration: null,
    ...over,
  });
  // Newest first, like the backend (created_at desc).
  const users: OrgUser[] = [
    person(7, "viewer", { status: "invited", last_login_at: null }),
    person(6, "researcher", { display_name: "Demo Researcher" }),
    person(5, "reviewer", { status: "disabled" }),
    person(4, "clinician", { display_name: "Demo Clinician", professional_registration: "REG 12345" }),
    person(3, "clinician", { display_name: "Demo Clinician Two", professional_registration: "REG 67890" }),
    {
      ...person(2, "admin", { display_name: "Demo Admin" }),
      id: userId,
      email: "admin@demo.example.org",
    },
    person(1, "owner", { display_name: "Demo Owner" }),
  ];
  const createdAt = new Map(users.map((u, i) => [u.id, iso(now - (30 + (users.length - i)) * DAY)]));

  // Audit log, newest first with monotonically decreasing numeric ids (as the backend's bigserial).
  const audit: AuditEntry[] = [];
  let nextAuditId = 1000;
  function record(entry: Omit<AuditEntry, "id" | "ts" | "org_id"> & { ts?: string }) {
    audit.unshift({ id: String(nextAuditId++), ts: entry.ts ?? iso(Date.now()), org_id: orgId, ...entry });
  }
  const seedTime = (i: number) => iso(now - i * 3 * 3_600_000);
  const seed: Omit<AuditEntry, "id" | "ts" | "org_id">[] = [];
  users.forEach((u) =>
    seed.push({
      actor_type: "user",
      actor_id: userId,
      action: "user.create",
      target_type: "user",
      target_id: u.id,
      before: null,
      after: { email: u.email, role: u.role, status: u.status },
    }),
  );
  ctx.devices.forEach((d, k) =>
    seed.push({
      actor_type: "user",
      actor_id: userId,
      action: d.revoked_at ? "device.revoke" : "device.enrol",
      target_type: "device",
      target_id: d.id,
      before: d.revoked_at ? { revoked_at: null } : null,
      after: d.revoked_at ? { revoked_at: d.revoked_at } : { platform: d.platform, label: d.label ?? null, n: k },
    }),
  );
  for (let i = 0; i < 40; i++) {
    const system = i % 4 === 0;
    seed.push({
      actor_type: system ? "system" : "user",
      actor_id: system ? null : userId,
      action: system ? "retention.sweep" : i % 3 === 0 ? "org.update" : "review.update",
      target_type: system ? "org" : i % 3 === 0 ? "org" : "review_item",
      target_id: system || i % 3 === 0 ? orgId : ctx.uuid(5000 + i),
      before: i % 3 === 0 && !system ? { settings: { timezone: "UTC" } } : null,
      after: i % 3 === 0 && !system ? { settings: { timezone: "Asia/Kathmandu" } } : { status: "done" },
    });
  }
  // `seed` is oldest first for ids; reverse so the first row printed is the newest.
  seed.forEach((e, i) => record({ ...e, ts: seedTime(seed.length - i) }));

  const findUser = (id: string) => {
    const u = users.find((x) => x.id === id);
    if (!u) throw problem(404, "not-found", "Not found", "User not found");
    return u;
  };

  const api: AdminApi = {
    async listParticipants(org, query = {}) {
      ctx.ensureOrg(org);
      const { slice, next_cursor, total } = ctx.page(participants, (p) => [p.created_at, p.id], query.cursor, query.limit);
      return {
        items: slice.map((p): Participant => {
          const item: Participant & { created_at?: string } = { ...p };
          delete item.created_at;
          item.withdrawn_at ??= ctx.withdrawn?.get(p.id) ?? null;
          return item;
        }),
        next_cursor,
        total,
      };
    },
    async listUsers(org, query = {}) {
      ctx.ensureOrg(org);
      const { slice, next_cursor } = ctx.page(users, (u) => [createdAt.get(u.id)!, u.id], query.cursor, query.limit);
      // The backend does not count users (total: null).
      return { items: slice.map((u) => ({ ...u })), next_cursor, total: null };
    },
    async getUser(org, id) {
      ctx.ensureOrg(org);
      return { ...findUser(id) };
    },
    async createUser(org, request: CreateUserRequest) {
      ctx.ensureOrg(org);
      const email = request.email.trim().toLowerCase();
      const params: [string, string][] = [];
      if (!/^[^@\s]+@[^@\s.][^@\s]*\.[^@\s.]+$/.test(email)) params.push(["email", "must be an e-mail address"]);
      if (request.password.length < 12) params.push(["password", "must be at least 12 characters"]);
      if ((request.display_name?.trim().length ?? 0) > 120) {
        params.push(["display_name", "must be at most 120 characters without line breaks"]);
      }
      if (request.role === "superadmin") params.push(["role", "superadmin is a global role, not an org role"]);
      if (params.length) throw problem(400, "validation-failed", "Validation failed", undefined, params);
      if (users.some((u) => u.email === email)) {
        throw problem(409, "email-taken", "Conflict", "a user with this e-mail address already exists");
      }
      const user: OrgUser = {
        id: ctx.uuid(8100 + users.length + audit.length),
        email,
        role: request.role,
        org_id: orgId,
        status: "active",
        last_login_at: null,
        display_name: request.display_name?.trim() || null,
        professional_registration: request.professional_registration?.trim() || null,
      };
      users.unshift(user);
      createdAt.set(user.id, iso(Date.now()));
      record({
        actor_type: "user",
        actor_id: userId,
        action: "user.create",
        target_type: "user",
        target_id: user.id,
        before: null,
        after: { email: user.email, role: user.role, status: user.status },
      });
      return { ...user };
    },
    async updateUser(org, id, patch: UpdateUserRequest) {
      ctx.ensureOrg(org);
      const user = findUser(id);
      if (patch.role === "superadmin") {
        throw problem(400, "validation-failed", "Validation failed", undefined, [["role", "superadmin is a global role, not an org role"]]);
      }
      if (patch.status === "invited") {
        throw problem(400, "validation-failed", "Validation failed", undefined, [["status", "must be active or disabled"]]);
      }
      if (id === userId) throw problem(403, "forbidden", "Forbidden");
      const role = patch.role ?? user.role;
      const status = patch.status ?? user.status;
      const lastOwner =
        user.role === "owner" &&
        user.status === "active" &&
        !(role === "owner" && status === "active") &&
        !users.some((u) => u.id !== id && u.role === "owner" && u.status === "active");
      if (lastOwner) {
        throw problem(409, "last-owner", "Conflict", "an organisation keeps at least one active owner");
      }
      const before = { role: user.role, status: user.status };
      user.role = role;
      user.status = status;
      if (patch.display_name !== undefined) user.display_name = patch.display_name?.trim() || null;
      if (patch.professional_registration !== undefined) {
        user.professional_registration = patch.professional_registration?.trim() || null;
      }
      record({
        actor_type: "user",
        actor_id: userId,
        action: "user.update",
        target_type: "user",
        target_id: id,
        before,
        after: { role, status },
      });
      return { ...user };
    },
    async listAudit(org, query: AuditQuery = {}) {
      ctx.ensureOrg(org);
      const params: [string, string][] = [];
      const time = (name: "from" | "to") => {
        const raw = query[name]?.trim();
        if (!raw) return null;
        const t = Date.parse(raw);
        if (Number.isNaN(t)) params.push([name, "must be an RFC 3339 timestamp"]);
        return Number.isNaN(t) ? null : t;
      };
      const from = time("from");
      const to = time("to");
      if (from !== null && to !== null && from > to) params.push(["from", "must not be after to"]);
      if (params.length) throw problem(400, "validation-failed", "Validation failed", undefined, params);
      const text = (v: string | undefined) => v?.trim() || undefined;
      const [actor, action, targetType, targetId] = [query.actor_id, query.action, query.target_type, query.target_id].map(text);
      const rows = audit.filter(
        (e) =>
          (!actor || e.actor_id === actor) &&
          (!action || e.action.startsWith(action)) &&
          (!targetType || e.target_type === targetType) &&
          (!targetId || e.target_id === targetId) &&
          (from === null || Date.parse(e.ts) >= from) &&
          (to === null || Date.parse(e.ts) <= to),
      );
      const { slice, next_cursor } = ctx.page(rows, (e) => [e.ts, e.id], query.cursor, query.limit);
      return { items: slice.map((e) => ({ ...e })), next_cursor, total: null };
    },
    async resetUserTotp(org, id, request) {
      ctx.ensureOrg(org);
      const target = findUser(id);
      // The mock caller is the admin `userId`: like the backend, yourself and higher ranks are refused first.
      const caller = findUser(userId);
      if (id === userId || RANK[target.role] > RANK[caller.role]) {
        throw problem(403, "totp-reset-forbidden", "Forbidden", "you cannot reset this user's two-step sign-in");
      }
      const reason = request.reason.trim();
      if ([...reason].length < MIN_TOTP_RESET_REASON_CHARS || [...reason].length > MAX_TOTP_RESET_REASON_CHARS) {
        throw problem(400, "validation-failed", "Validation failed", undefined, [
          ["reason", `must be ${MIN_TOTP_RESET_REASON_CHARS} to ${MAX_TOTP_RESET_REASON_CHARS} characters`],
        ]);
      }
      if (request.password !== MOCK_STEP_UP_PASSWORD) {
        record({
          actor_type: "user",
          actor_id: userId,
          action: "user.totp_reset_failed",
          target_type: "user",
          target_id: id,
          before: null,
          after: null,
        });
        throw problem(401, "reauthentication-failed", "Re-authentication failed");
      }
      record({
        actor_type: "user",
        actor_id: userId,
        action: "user.totp_reset_by_admin",
        target_type: "user",
        target_id: id,
        before: null,
        after: { reason },
      });
    },
  };

  // WP-C.18: the record mock (participant create, encounters, attachments) shares this participant list.
  registerMockRecordStore(
    { orgId, userId, now, details: ctx.details, participants, page: ctx.page, encounters: [], attachments: [], seeded: false },
    api.listParticipants,
  );
  return api;
}
