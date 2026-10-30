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
