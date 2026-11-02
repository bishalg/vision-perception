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

/**
 * A prefetch the browser labels as such (`Purpose` / `Sec-Purpose`). Next strips its own router headers (`RSC`,
 * `Next-Router-Prefetch`, `Next-Router-Segment-Prefetch`) and the `_rsc` query from the request the proxy sees, so
 * the App Router's own prefetches are indistinguishable from navigations here.
 */
function isPrefetch(request: NextRequest): boolean {
  const h = request.headers;
  return h.has("next-router-prefetch") || h.get("purpose") === "prefetch" || h.get("sec-purpose")?.startsWith("prefetch") === true;
}

/** A server action call (`POST` with the `Next-Action` header, which Next leaves visible to the proxy). */
function isServerAction(request: NextRequest): boolean {
  return request.headers.has("next-action");
}

/**
 * HIP-2: remembers why the session ended for a few minutes, so a request that arrives after the cookies are gone (a
 * second tab, a router prefetch that ended the session before the click) still lands on `/login?reason=...`.
 */
const LOGOFF_REASON_COOKIE = "curv_logoff_reason";
const LOGOFF_REASON_MAX_AGE_SEC = 300;

function loginUrl(request: NextRequest, reason: string | undefined): URL {
  const { pathname, search } = request.nextUrl;
  const url = new URL("/login", request.url);
  if (reason) url.searchParams.set("reason", reason);
  if (pathname !== "/") url.searchParams.set("next", pathname + search);
  return url;
}

/**
 * HIP-2: ends an idle or expired session on any non-prefetch request. Nothing is refreshed or revived: the session
 * cookies are cleared and the client lands on `/login?reason=...` (wave 1 defect 1):
 * - server actions: Next's action-redirect protocol (`x-action-redirect`, empty body); the router navigates to the
 *   login page instead of running the action;
 * - route handlers (`/api/...`, fetched or downloaded by the page): `401` problem JSON;
 * - everything else (document loads, RSC navigations and router prefetches): a 307 to the login page, which the
 *   browser or the App Router's `fetch` follows.
 */
function endSession(request: NextRequest, verdict: "idle" | "expired"): NextResponse {
  const url = loginUrl(request, verdict);
  const target = url.pathname + url.search;
  let response: NextResponse;
  if (isServerAction(request)) {
    response = withCsp(request, () =>
      new NextResponse(null, { status: 200, headers: { "x-action-redirect": `${target};replace`, "content-type": "text/plain" } }),
    );
  } else if (request.nextUrl.pathname.startsWith("/api/")) {
    response = withCsp(request, () =>
      NextResponse.json(
        { type: "about:blank", title: "Signed out", status: 401, detail: `session ${verdict}`, login: target },
        { status: 401, headers: { "content-type": "application/problem+json" } },
      ),
    );
  } else {
    response = withCsp(request, () => NextResponse.redirect(url));
  }
  response.headers.set("Cache-Control", "no-store");
  for (const name of SESSION_COOKIE_NAMES) response.cookies.delete(name);
  response.cookies.set(LOGOFF_REASON_COOKIE, verdict, {
    httpOnly: true,
    sameSite: "lax",
    secure: cookieSecure(),
    path: "/",
    maxAge: LOGOFF_REASON_MAX_AGE_SEC,
  });
  return response;
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

  // HIP-2 automatic logoff. The verdict is computed for every signed-in, non-public request. An idle or expired
  // session never proceeds and never mints tokens: every request ends it (cookies cleared, the client lands on
  // /login?reason=...), except a browser-labelled prefetch, which gets an empty 204 (nothing rendered or refreshed).
  let active = false;
  if ((hasAccess || refreshToken) && !isPublicPath(pathname)) {
    const verdict = evaluate(
      parseStamp(request.cookies.get(LAST_SEEN_COOKIE)?.value),
      parseStamp(request.cookies.get(LOGIN_AT_COOKIE)?.value),
      Date.now(),
    );
    active = verdict === "ok";
    if (verdict !== "ok") {
      if (isPrefetch(request)) {
        const response = withCsp(request, () => new NextResponse(null, { status: 204 }));
        response.headers.set("Cache-Control", "no-store");
        return response;
      }
      return endSession(request, verdict);
    }
  }

  if (decision === "redirect-home") return withCsp(request, () => NextResponse.redirect(new URL("/", request.url)));

  if (decision === "redirect-login") {
    // A session ended by HIP-2 moments ago keeps its reason (see LOGOFF_REASON_COOKIE).
    const ended = request.cookies.get(LOGOFF_REASON_COOKIE)?.value;
    const reason = ended === "idle" || ended === "expired" ? ended : undefined;
    const response = withCsp(request, () => NextResponse.redirect(loginUrl(request, reason)));
    if (ended !== undefined) response.cookies.delete(LOGOFF_REASON_COOKIE);
    return response;
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
