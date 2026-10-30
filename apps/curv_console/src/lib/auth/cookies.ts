/** Cookie names and options for the console session. Tokens live only in httpOnly cookies. */
import type { TokenPair } from "@/lib/api";

export const ACCESS_COOKIE = "curv_access";
export const REFRESH_COOKIE = "curv_refresh";
export const ORG_COOKIE = "curv_org";

/** Refresh-cookie lifetime. Must not exceed the backend's refresh-token lifetime (assumed 7 days). */
export const REFRESH_MAX_AGE_SEC = 7 * 24 * 60 * 60;

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
    // The superadmin has no home org (org_id null), so no org cookie.
    ...(pair.user.org_id === null
      ? []
      : [{ name: ORG_COOKIE, value: pair.user.org_id, options: { ...base, maxAge: REFRESH_MAX_AGE_SEC } }]),
  ];
}

export const SESSION_COOKIE_NAMES = [ACCESS_COOKIE, REFRESH_COOKIE, ORG_COOKIE] as const;
