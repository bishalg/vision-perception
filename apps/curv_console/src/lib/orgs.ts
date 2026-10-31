/** Recently visited orgs for the org switcher. The backend has no "list my orgs" endpoint (see README). */

export const RECENT_ORGS_COOKIE = "curv_recent_orgs";
export const MAX_RECENT_ORGS = 5;

const UUID_RE = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

export function isOrgId(value: string | null | undefined): value is string {
  return typeof value === "string" && UUID_RE.test(value);
}

/** `/{uuid}/...` -> uuid; other paths -> null. */
export function orgFromPath(pathname: string): string | null {
  const first = pathname.split("/")[1] ?? "";
  return isOrgId(first) ? first.toLowerCase() : null;
}

export function parseRecentOrgs(value: string | null | undefined): string[] {
  if (!value) return [];
  return value.split(",").filter(isOrgId).slice(0, MAX_RECENT_ORGS);
}

export function rememberOrg(value: string | null | undefined, orgId: string): string {
  const id = orgId.toLowerCase();
  return [id, ...parseRecentOrgs(value).filter((o) => o !== id)].slice(0, MAX_RECENT_ORGS).join(",");
}
