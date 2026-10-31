/**
 * Which controls to show for a role. Mirrors the backend RBAC matrix (`asd-backend auth::rbac::MATRIX`) only to
 * hide buttons the user cannot use; the backend is the authority and answers 403/404 regardless.
 */
import type { Role } from "@/lib/api";

export type UiAction = "review.write" | "device.read" | "enrol.create" | "device.revoke";

const MATRIX: Record<UiAction, readonly Role[]> = {
  "review.write": ["owner", "admin", "clinician", "reviewer"],
  "device.read": ["owner", "admin", "clinician"],
  "enrol.create": ["owner", "admin", "clinician"],
  "device.revoke": ["owner", "admin"],
};

export function can(role: Role | undefined, action: UiAction): boolean {
  if (!role) return false;
  return role === "superadmin" || MATRIX[action].includes(role);
}
