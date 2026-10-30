import "server-only";
import { cookies } from "next/headers";
import { redirect } from "next/navigation";
import { getApi, type CurvApi } from "@/lib/api";
import { ACCESS_COOKIE, ORG_COOKIE, REFRESH_COOKIE, SESSION_COOKIE_NAMES, cookieSecure, sessionCookies } from "./cookies";
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
