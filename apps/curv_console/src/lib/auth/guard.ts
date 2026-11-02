/** Pure routing decision used by proxy.ts, kept separate so it is unit-testable. */

export type GuardDecision = "allow" | "redirect-login" | "refresh" | "redirect-home";

const PUBLIC_PATHS = ["/login"];

export function isPublicPath(pathname: string): boolean {
  return PUBLIC_PATHS.some((p) => pathname === p || pathname.startsWith(`${p}/`));
}

export function decideGuard(pathname: string, hasAccess: boolean, hasRefresh: boolean): GuardDecision {
  if (isPublicPath(pathname)) return hasAccess ? "redirect-home" : "allow";
  if (hasAccess) return "allow";
  return hasRefresh ? "refresh" : "redirect-login";
}

/** Only same-site relative paths are accepted as post-login targets (prevents open redirects). */
export function safeNextPath(next: string | null | undefined): string | null {
  if (!next) return null;
  if (!next.startsWith("/") || next.startsWith("//") || next.startsWith("/\\")) return null;
  if (next.startsWith("/login")) return null;
  return next;
}

/**
 * WP-C.17 (AUTH-13): where the org layout sends a user whose role requires two-step sign-in and who is past the
 * deadline (`totp_enforcement === "required"`): the account security page, the only place they can still act.
 * Returns null when no redirect applies, including on that page and below it (no loop). `pathname` is the
 * `x-pathname` request header set by proxy.ts (null when absent: redirect, the backend would refuse the page anyway).
 */
export function enforcementRedirect(
  profile: { totp_enforcement?: "not_required" | "grace" | "required" } | null | undefined,
  pathname: string | null,
  org: string,
): string | null {
  if (profile?.totp_enforcement !== "required") return null;
  const security = `/${encodeURIComponent(org)}/settings/security`;
  if (pathname !== null && (pathname === security || pathname.startsWith(`${security}/`))) return null;
  return `${security}?required=1`;
}
