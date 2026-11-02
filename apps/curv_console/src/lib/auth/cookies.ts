/** Cookie names and options for the console session. Tokens live only in httpOnly cookies. */
import { ROLES, type Role, type TokenPair } from "@/lib/api";
import { SESSION_MAX_MS } from "./session-policy";

export const ACCESS_COOKIE = "curv_access";
export const REFRESH_COOKIE = "curv_refresh";
export const ORG_COOKIE = "curv_org";
/** The signed-in role, used only to hide controls the user cannot use; the backend enforces RBAC. */
export const ROLE_COOKIE = "curv_role";

/** Backend refresh-token lifetime (30 days). */
const BACKEND_REFRESH_TTL_SEC = 30 * 24 * 60 * 60;

/**
 * Refresh-cookie lifetime: min(backend TTL, absolute session length) (HIP-2). The browser drops the cookies at the
 * absolute limit even if proxy.ts never sees the request.
 */
export const REFRESH_MAX_AGE_SEC = Math.min(BACKEND_REFRESH_TTL_SEC, SESSION_MAX_MS / 1000);

/** Last document load (epoch ms), refreshed by proxy.ts; drives the idle timeout. */
export const LAST_SEEN_COOKIE = "curv_last_seen";
/** Sign-in time (epoch ms), set once at login and never refreshed; drives the absolute limit. */
export const LOGIN_AT_COOKIE = "curv_login_at";

export interface CookieSpec {
  name: string;
  value: string;
  options: {
    httpOnly: true;
    sameSite: "lax";
    secure: boolean;
    path: "/";
    maxAge: number;
  };
}

export function cookieSecure(env: Record<string, string | undefined> = process.env): boolean {
  return env.CURV_COOKIE_SECURE === "true" || env.NODE_ENV === "production";
}

export function sessionCookies(pair: TokenPair, secure: boolean): CookieSpec[] {
  const base = { httpOnly: true, sameSite: "lax", secure, path: "/" } as const;
  return [
    // The access cookie expires with the access token, which is what triggers a refresh in proxy.ts.
    { name: ACCESS_COOKIE, value: pair.access_token, options: { ...base, maxAge: pair.expires_in } },
    { name: REFRESH_COOKIE, value: pair.refresh_token, options: { ...base, maxAge: REFRESH_MAX_AGE_SEC } },
    { name: ROLE_COOKIE, value: pair.user.role, options: { ...base, maxAge: REFRESH_MAX_AGE_SEC } },
    // The superadmin has no home org (org_id null), so no org cookie.
    ...(pair.user.org_id === null
      ? []
      : [{ name: ORG_COOKIE, value: pair.user.org_id, options: { ...base, maxAge: REFRESH_MAX_AGE_SEC } }]),
  ];
}

/** The two logoff-policy stamps. `loginAt` is only passed at sign-in: token rotation must not extend the session. */
export function stampCookies(secure: boolean, now: number, loginAt?: number): CookieSpec[] {
  const base = { httpOnly: true, sameSite: "lax", secure, path: "/", maxAge: REFRESH_MAX_AGE_SEC } as const;
  return [
    { name: LAST_SEEN_COOKIE, value: String(now), options: base },
    ...(loginAt === undefined ? [] : [{ name: LOGIN_AT_COOKIE, value: String(loginAt), options: base }]),
  ];
}

export const SESSION_COOKIE_NAMES = [
  ACCESS_COOKIE,
  REFRESH_COOKIE,
  ORG_COOKIE,
  ROLE_COOKIE,
  LAST_SEEN_COOKIE,
  LOGIN_AT_COOKIE,
] as const;

export function readRole(value: string | undefined): Role | undefined {
  return value && (ROLES as readonly string[]).includes(value) ? (value as Role) : undefined;
}
