import { NextResponse, type NextRequest } from "next/server";
import { getApi } from "@/lib/api";
import {
  ACCESS_COOKIE,
  LAST_SEEN_COOKIE,
  LOGIN_AT_COOKIE,
  REFRESH_COOKIE,
  REFRESH_MAX_AGE_SEC,
  SESSION_COOKIE_NAMES,
  cookieSecure,
  sessionCookies,
  stampCookies,
} from "@/lib/auth/cookies";
import { decideGuard, isPublicPath } from "@/lib/auth/guard";
import { evaluate, parseStamp } from "@/lib/auth/session-policy";
import { HSTS, buildCsp, newNonce, sendHsts } from "@/lib/csp";
import { RECENT_ORGS_COOKIE, orgFromPath, rememberOrg } from "@/lib/orgs";

/** Remember the org of an org-scoped page for the org switcher (ids only, httpOnly). */
function trackOrg(request: NextRequest, response: NextResponse): NextResponse {
  const org = orgFromPath(request.nextUrl.pathname);
  if (!org) return response;
  const current = request.cookies.get(RECENT_ORGS_COOKIE)?.value;
  const next = rememberOrg(current, org);
  if (next !== current) {
    response.cookies.set(RECENT_ORGS_COOKIE, next, {
      httpOnly: true,
      sameSite: "lax",
      secure: cookieSecure(),
      path: "/",
      maxAge: REFRESH_MAX_AGE_SEC,
    });
  }
  return response;
}

/**
 * CSP-1: the policy goes on the request headers (Next reads the nonce from the request's CSP header while
 * rendering and stamps it on framework scripts; `x-nonce` is forwarded for completeness) and on the response,
 * redirects included. CSP-2: HSTS goes on every proxy response in production.
 */
function withCsp(request: NextRequest, respond: (headers: Headers) => NextResponse): NextResponse {
  const nonce = newNonce();
  const csp = buildCsp(nonce, process.env.NODE_ENV === "development");
  const headers = new Headers(request.headers);
  headers.set("x-nonce", nonce);
  headers.set("x-pathname", request.nextUrl.pathname);
  headers.set("Content-Security-Policy", csp);
  const response = respond(headers);
  response.headers.set("Content-Security-Policy", csp);
  if (sendHsts()) response.headers.set("Strict-Transport-Security", HSTS);
  return response;
}

/** A browser document load (address bar, reload, link): not an RSC / prefetch / fetch / server-action request. */
function isDocumentRequest(request: NextRequest): boolean {
  return request.headers.get("sec-fetch-dest") === "document" && !request.headers.has("rsc");
}

/** Next's link prefetch: an idle open tab must not keep itself alive through these. */
function isPrefetch(request: NextRequest): boolean {
  const h = request.headers;
  return h.has("next-router-prefetch") || h.get("purpose") === "prefetch" || h.get("sec-purpose")?.startsWith("prefetch") === true;
}

/**
 * Refresh `curv_last_seen` on any authenticated, non-prefetch request whose session is still within policy
 * (document loads, RSC navigations, server actions), so client-side activity counts as activity. An idle or expired
 * session is never revived here. `curv_login_at` is never touched.
 */
function touch(request: NextRequest, response: NextResponse, active: boolean): NextResponse {
  if (!active || isPrefetch(request)) return response;
  for (const c of stampCookies(cookieSecure(), Date.now())) response.cookies.set(c.name, c.value, c.options);
  return response;
}

export async function proxy(request: NextRequest): Promise<NextResponse> {
  const { pathname } = request.nextUrl;
  const hasAccess = Boolean(request.cookies.get(ACCESS_COOKIE)?.value);
  const refreshToken = request.cookies.get(REFRESH_COOKIE)?.value;
  const decision = decideGuard(pathname, hasAccess, Boolean(refreshToken));

  // HIP-2 automatic logoff. The verdict is computed for every signed-in, non-public request; only document loads
  // are redirected (clearing the cookies, with the CSP and a `reason` banner on /login).
  let active = false;
  if ((hasAccess || refreshToken) && !isPublicPath(pathname)) {
    const verdict = evaluate(
      parseStamp(request.cookies.get(LAST_SEEN_COOKIE)?.value),
      parseStamp(request.cookies.get(LOGIN_AT_COOKIE)?.value),
      Date.now(),
    );
    active = verdict === "ok";
    if (verdict !== "ok" && isDocumentRequest(request)) {
      const url = new URL("/login", request.url);
      url.searchParams.set("reason", verdict);
      if (pathname !== "/") url.searchParams.set("next", pathname + request.nextUrl.search);
      const response = withCsp(request, () => NextResponse.redirect(url));
      for (const name of SESSION_COOKIE_NAMES) response.cookies.delete(name);
      return response;
    }
  }

  if (decision === "redirect-home") return withCsp(request, () => NextResponse.redirect(new URL("/", request.url)));

  if (decision === "redirect-login") {
    const url = new URL("/login", request.url);
    if (pathname !== "/") url.searchParams.set("next", pathname + request.nextUrl.search);
    return withCsp(request, () => NextResponse.redirect(url));
  }

  if (decision === "refresh" && refreshToken) {
    try {
      const pair = await getApi().refresh(refreshToken);
      const specs = sessionCookies(pair, cookieSecure());
      // Make the rotated tokens visible to this request's server components, and persist them for the browser.
      for (const c of specs) request.cookies.set(c.name, c.value);
      const response = withCsp(request, (headers) => NextResponse.next({ request: { headers } }));
      for (const c of specs) response.cookies.set(c.name, c.value, c.options);
      return touch(request, trackOrg(request, response), active);
    } catch {
      const response = withCsp(request, () => NextResponse.redirect(new URL("/login", request.url)));
      for (const name of [ACCESS_COOKIE, REFRESH_COOKIE]) response.cookies.delete(name);
      return response;
    }
  }

  return touch(request, trackOrg(request, withCsp(request, (headers) => NextResponse.next({ request: { headers } }))), active);
}

export const config = {
  matcher: ["/((?!_next/static|_next/image|favicon.ico).*)"],
};
