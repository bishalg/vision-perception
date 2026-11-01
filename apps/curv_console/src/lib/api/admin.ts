/**
 * Administration API (WP-C.3): participants list, org users and the audit log. zod schemas for the DTOs of
 * `docs/spec/schemas/asd/api-v1.json` (checked by `contract.test.ts`), the `AdminApi` interface and the live
 * HTTP implementation. The console holds no RBAC logic beyond hiding controls: the backend answers 403/404.
 */
import { z } from "zod";
import { roleSchema, userStatusSchema, type PageQuery } from "./types";
import type { LiveRequest } from "./clinical";

const str = z.string().nullable().optional();

// ---- participants -----------------------------------------------------------

/** `GET /orgs/{id}/participants` item (CON-20). `has_patient_ref` is absent without `PatientRefRead`. */
export const participantSchema = z.object({
  id: z.string(),
  org_id: z.string(),
  participant_token: z.string(),
  age_bracket_at_enrol: str,
  language: str,
  session_count: z.number().int().min(0),
  last_session_at: str,
  withdrawn_at: str,
  has_patient_ref: z.boolean().nullable().optional(),
});
export type Participant = z.infer<typeof participantSchema>;

const pageFields = {
  next_cursor: z.string().nullable().optional(),
  total: z.number().int().min(0).nullable().optional(),
};

export const participantPageSchema = z.object({ items: z.array(participantSchema), ...pageFields });
export type ParticipantPage = z.infer<typeof participantPageSchema>;

// ---- users ------------------------------------------------------------------

/** `GET /orgs/{id}/users` item. */
export const orgUserSchema = z.object({
  id: z.string(),
  email: z.string(),
  role: roleSchema,
  org_id: z.string(),
  status: userStatusSchema,
  last_login_at: str,
  display_name: str,
  professional_registration: str,
});
export type OrgUser = z.infer<typeof orgUserSchema>;

export const orgUserPageSchema = z.object({ items: z.array(orgUserSchema), ...pageFields });
export type OrgUserPage = z.infer<typeof orgUserPageSchema>;

/** `POST /orgs/{id}/users`. The actor chooses the password; no password set or reset endpoint exists. */
export const createUserRequestSchema = z.object({
  email: z.string(),
  role: roleSchema,
  password: z.string(),
  display_name: str,
  professional_registration: str,
});
export type CreateUserRequest = z.infer<typeof createUserRequestSchema>;

/** `PATCH /orgs/{id}/users/{user_id}`: absent = unchanged, `""` clears a profile field; status active or disabled. */
export const updateUserRequestSchema = z.object({
  role: roleSchema.nullable().optional(),
  status: userStatusSchema.nullable().optional(),
  display_name: str,
  professional_registration: str,
});
export type UpdateUserRequest = z.infer<typeof updateUserRequestSchema>;

// ---- audit ------------------------------------------------------------------

export const ACTOR_TYPES = ["user", "device", "system"] as const;
export const actorTypeSchema = z.enum(ACTOR_TYPES);

/** `GET /orgs/{id}/audit` item. `before`/`after` are redacted JSON snapshots of arbitrary shape. */
export const auditEntrySchema = z.object({
  id: z.string(),
  ts: z.string(),
  actor_type: actorTypeSchema,
  actor_id: str,
  org_id: str,
  action: z.string(),
  target_type: str,
  target_id: str,
  before: z.unknown().optional(),
  after: z.unknown().optional(),
});
export type AuditEntry = z.infer<typeof auditEntrySchema>;

export const auditPageSchema = z.object({ items: z.array(auditEntrySchema), ...pageFields });
export type AuditPage = z.infer<typeof auditPageSchema>;

export interface AdminApi {
  listParticipants(orgId: string, query?: PageQuery): Promise<ParticipantPage>;
  listUsers(orgId: string, query?: PageQuery): Promise<OrgUserPage>;
  createUser(orgId: string, request: CreateUserRequest): Promise<OrgUser>;
  updateUser(orgId: string, userId: string, patch: UpdateUserRequest): Promise<OrgUser>;
  listAudit(orgId: string, query?: PageQuery): Promise<AuditPage>;
}

export function createLiveAdmin(request: LiveRequest, buildQuery: (q: object) => string): AdminApi {
  const enc = encodeURIComponent;
  return {
    listParticipants: (orgId, query) =>
      request(`/orgs/${enc(orgId)}/participants${buildQuery(query ?? {})}`, participantPageSchema),
    listUsers: (orgId, query) => request(`/orgs/${enc(orgId)}/users${buildQuery(query ?? {})}`, orgUserPageSchema),
    createUser: (orgId, body) => request(`/orgs/${enc(orgId)}/users`, orgUserSchema, { method: "POST", body }),
    updateUser: (orgId, userId, patch) =>
      request(`/orgs/${enc(orgId)}/users/${enc(userId)}`, orgUserSchema, { method: "PATCH", body: patch }),
    listAudit: (orgId, query) => request(`/orgs/${enc(orgId)}/audit${buildQuery(query ?? {})}`, auditPageSchema),
  };
}
