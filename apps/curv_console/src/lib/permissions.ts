/**
 * Which controls to show for a role. Mirrors the backend RBAC matrix (`asd-backend auth::rbac::MATRIX`) only to
 * hide buttons the user cannot use; the backend is the authority and answers 403/404 regardless.
 */
import type { Role } from "@/lib/api";

export type UiAction =
  | "review.write"
  | "device.read"
  | "enrol.create"
  | "device.revoke"
  // WP-C.13 (mirrors asd-backend rbac MATRIX: WorklistRead, ClinicalReviewWrite, ClinicalSign, ClinicalCosign,
  // ClinicalContentRead, OrgUpdate). Superadmin never gets clinical write/content (RBAC-3), unlike the others.
  | "worklist.read"
  | "clinical.write"
  | "clinical.sign"
  | "clinical.cosign"
  | "clinical.content"
  | "org.update"
  // WP-C.3 (UserRead, UserManage, AuditRead). Participant lists are open to every org role (ParticipantRead).
  | "user.read"
  | "user.manage"
  | "audit.read"
  // WP-I.8 exports and delivery destinations (ExportRead / ExportDeidentified / ExportIdentified / IntegrationRead /
  // IntegrationManage). Superadmin reads but never exports, downloads or manages integrations (RBAC-3).
  | "export.read"
  | "export.create"
  | "export.identified"
  | "export.download"
  | "integration.read"
  | "integration.manage"
  // WP-C.4 global config documents: ConfigRead; ConfigGlobalManage (create/approve/publish/retire) is superadmin only.
  | "config.read"
  | "config.manage"
  // WP-C.5 composing a signed config bundle (ConfigPublish): owners and admins for their org, superadmin any.
  | "config.publish"
  // WP-B.9 erasure requests, participant erasure and the retention sweep (ParticipantErase). Unlike the clinical
  // actions the backend lets the superadmin through (not clinical content), so it is not in NO_SUPERADMIN.
  | "participant.erase"
  // WP-C.14 (ClinicalNoteWrite, PatientRefRead, PatientRefWrite). Notes and disclosures are read with
  // `clinical.content`. The MRN actions are clinical content: never for the superadmin (RBAC-3).
  | "note.write"
  | "patientref.read"
  | "patientref.write";

const MATRIX: Record<UiAction, readonly Role[]> = {
  "review.write": ["owner", "admin", "clinician", "reviewer"],
  "device.read": ["owner", "admin", "clinician"],
  "enrol.create": ["owner", "admin", "clinician"],
  "device.revoke": ["owner", "admin"],
  "worklist.read": ["owner", "admin", "clinician"],
  "clinical.write": ["clinician"],
  "clinical.sign": ["clinician"],
  "clinical.cosign": ["clinician"],
  "clinical.content": ["owner", "clinician"],
  "org.update": ["owner", "admin"],
  "user.read": ["owner", "admin"],
  "user.manage": ["owner", "admin"],
  "audit.read": ["owner", "admin"],
  "export.read": ["owner", "admin", "clinician", "researcher"],
  "export.create": ["owner", "admin", "clinician", "researcher"],
  "export.identified": ["owner", "clinician"],
  "export.download": ["owner", "admin", "clinician", "researcher"],
  "integration.read": ["owner", "admin"],
  "integration.manage": ["owner", "admin"],
  "config.read": ["owner", "admin", "clinician", "researcher"],
  "config.publish": ["owner", "admin"],
  "config.manage": [],
  "participant.erase": ["owner", "admin"],
  "note.write": ["clinician"],
  "patientref.read": ["owner", "admin", "clinician"],
  "patientref.write": ["owner", "admin", "clinician"],
};

/** RBAC-3: the superadmin short-circuit does not apply to clinical content actions. */
const NO_SUPERADMIN: readonly UiAction[] = [
  "clinical.write",
  "clinical.sign",
  "clinical.cosign",
  "clinical.content",
  "export.create",
  "export.identified",
  "export.download",
  "integration.manage",
  "note.write",
  "patientref.read",
  "patientref.write",
];

export function can(role: Role | undefined, action: UiAction): boolean {
  if (!role) return false;
  if (role === "superadmin") return !NO_SUPERADMIN.includes(action);
  return MATRIX[action].includes(role);
}

const RANK: Record<Role, number> = { superadmin: 3, owner: 2, admin: 1, clinician: 0, researcher: 0, reviewer: 0, viewer: 0 };

/**
 * Org roles the actor may grant on create or edit (mirrors `rbac::can_assign` and `rbac::can_provision`): up to its
 * own rank, and no role holding clinical-content actions the actor lacks (the actor knows or chose the password).
 * Owners provision everything; admins get no owner or clinician; superadmin only roles without clinical actions.
 */
export function assignableRoles(actor: Role | undefined): Role[] {
  switch (actor) {
    case "owner":
      return ["owner", "admin", "clinician", "researcher", "reviewer", "viewer"];
    case "admin":
      return ["admin", "researcher", "reviewer", "viewer"];
    case "superadmin":
      return ["reviewer", "viewer"];
    default:
      return [];
  }
}

/** Whether the actor may edit a user holding `target` (rank rule of `rbac::can_assign`); the backend still decides. */
export function canEditUserRole(actor: Role | undefined, target: Role): boolean {
  return !!actor && target !== "superadmin" && RANK[target] <= RANK[actor];
}
