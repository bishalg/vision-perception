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
  | "org.update";

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
};

/** RBAC-3: the superadmin short-circuit does not apply to clinical content actions. */
const NO_SUPERADMIN: readonly UiAction[] = ["clinical.write", "clinical.sign", "clinical.cosign", "clinical.content"];

export function can(role: Role | undefined, action: UiAction): boolean {
  if (!role) return false;
  if (role === "superadmin") return !NO_SUPERADMIN.includes(action);
  return MATRIX[action].includes(role);
}
