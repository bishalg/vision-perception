/** Pure routing decision used by proxy.ts, kept separate so it is unit-testable. */

export type GuardDecision = "allow" | "redirect-login" | "refresh" | "redirect-home";

/** Sign-in pages: open without a session, and a signed-in user is sent home from them. */
const PUBLIC_PATHS = ["/login"];

/**
 * WP-C.19 (SHR-9): pages and routes for recipients without an account (the public share page and its download
 * route). Open to everyone, signed in or not: no auth guard, no `redirect-home` (a clinician opening a link they were
 * sent stays on it), no HIP-2 verdict or `curv_last_seen` refresh (they are public paths for proxy.ts).
 */
const ANONYMOUS_PATHS = ["/share", "/api/share"];

const under = (pathname: string, prefixes: readonly string[]) =>
  prefixes.some((p) => pathname === p || pathname.startsWith(`${p}/`));

export function isAnonymousPath(pathname: string): boolean {
  return under(pathname, ANONYMOUS_PATHS);
}

/** Paths reachable without a session: the sign-in pages and the anonymous share paths. */
export function isPublicPath(pathname: string): boolean {
  return under(pathname, PUBLIC_PATHS) || isAnonymousPath(pathname);
}

export function decideGuard(pathname: string, hasAccess: boolean, hasRefresh: boolean): GuardDecision {
  if (isAnonymousPath(pathname)) return "allow";
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
