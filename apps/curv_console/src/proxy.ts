import { NextResponse, type NextRequest } from "next/server";
import { getApi } from "@/lib/api";
import { ACCESS_COOKIE, REFRESH_COOKIE, REFRESH_MAX_AGE_SEC, cookieSecure, sessionCookies } from "@/lib/auth/cookies";
import { decideGuard } from "@/lib/auth/guard";
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
  headers.set("Content-Security-Policy", csp);
  const response = respond(headers);
  response.headers.set("Content-Security-Policy", csp);
  if (sendHsts()) response.headers.set("Strict-Transport-Security", HSTS);
  return response;
}

export async function proxy(request: NextRequest): Promise<NextResponse> {
  const { pathname } = request.nextUrl;
  const hasAccess = Boolean(request.cookies.get(ACCESS_COOKIE)?.value);
  const refreshToken = request.cookies.get(REFRESH_COOKIE)?.value;
  const decision = decideGuard(pathname, hasAccess, Boolean(refreshToken));

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
      return trackOrg(request, response);
    } catch {
      const response = withCsp(request, () => NextResponse.redirect(new URL("/login", request.url)));
      for (const name of [ACCESS_COOKIE, REFRESH_COOKIE]) response.cookies.delete(name);
      return response;
    }
  }

  return trackOrg(request, withCsp(request, (headers) => NextResponse.next({ request: { headers } })));
}

export const config = {
  matcher: ["/((?!_next/static|_next/image|favicon.ico).*)"],
};
