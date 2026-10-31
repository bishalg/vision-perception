import "server-only";
import { cookies } from "next/headers";
import { redirect } from "next/navigation";
import { getApi, type CurvApi, type Role } from "@/lib/api";
import { RECENT_ORGS_COOKIE, parseRecentOrgs } from "@/lib/orgs";
import {
  ACCESS_COOKIE,
  ORG_COOKIE,
  REFRESH_COOKIE,
  ROLE_COOKIE,
  SESSION_COOKIE_NAMES,
  cookieSecure,
  readRole,
  sessionCookies,
} from "./cookies";
import type { TokenPair } from "@/lib/api";

export async function writeSession(pair: TokenPair): Promise<void> {
  const jar = await cookies();
  const specs = sessionCookies(pair, cookieSecure());
  for (const c of specs) jar.set(c.name, c.value, c.options);
  // A superadmin (no org) must not inherit an earlier user's org cookie.
  if (!specs.some((c) => c.name === ORG_COOKIE)) jar.delete(ORG_COOKIE);
}

export async function clearSession(): Promise<void> {
  const jar = await cookies();
  for (const name of SESSION_COOKIE_NAMES) jar.delete(name);
  jar.delete(RECENT_ORGS_COOKIE);
}

/** The signed-in role, for hiding controls only (the backend enforces RBAC). */
export async function currentRole(): Promise<Role | undefined> {
  return readRole((await cookies()).get(ROLE_COOKIE)?.value);
}

/** Orgs visited recently (most recent first), maintained by proxy.ts. */
export async function recentOrgs(): Promise<string[]> {
  return parseRecentOrgs((await cookies()).get(RECENT_ORGS_COOKIE)?.value);
}

export async function readRefreshToken(): Promise<string | undefined> {
  return (await cookies()).get(REFRESH_COOKIE)?.value;
}

export async function homeOrg(): Promise<string | undefined> {
  return (await cookies()).get(ORG_COOKIE)?.value;
}

/** API client bound to the caller's access token. Redirects to /login when there is none. */
export async function authedApi(): Promise<CurvApi> {
  const token = (await cookies()).get(ACCESS_COOKIE)?.value;
  if (!token) redirect("/login");
  return getApi(token);
}

/** Access token for route handlers that proxy binary responses. */
export async function accessToken(): Promise<string | undefined> {
  return (await cookies()).get(ACCESS_COOKIE)?.value;
}
