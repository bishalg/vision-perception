import { NextResponse, type NextRequest } from "next/server";
import { getApi } from "@/lib/api";
import { ACCESS_COOKIE, REFRESH_COOKIE, REFRESH_MAX_AGE_SEC, cookieSecure, sessionCookies } from "@/lib/auth/cookies";
import { decideGuard } from "@/lib/auth/guard";
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

export async function proxy(request: NextRequest) {
  const { pathname } = request.nextUrl;
  const hasAccess = Boolean(request.cookies.get(ACCESS_COOKIE)?.value);
  const refreshToken = request.cookies.get(REFRESH_COOKIE)?.value;
  const decision = decideGuard(pathname, hasAccess, Boolean(refreshToken));

  if (decision === "redirect-home") return NextResponse.redirect(new URL("/", request.url));

  if (decision === "redirect-login") {
    const url = new URL("/login", request.url);
    if (pathname !== "/") url.searchParams.set("next", pathname + request.nextUrl.search);
    return NextResponse.redirect(url);
  }

  if (decision === "refresh" && refreshToken) {
    try {
      const pair = await getApi().refresh(refreshToken);
      const specs = sessionCookies(pair, cookieSecure());
      // Make the rotated tokens visible to this request's server components, and persist them for the browser.
      for (const c of specs) request.cookies.set(c.name, c.value);
      const response = NextResponse.next({ request: { headers: request.headers } });
      for (const c of specs) response.cookies.set(c.name, c.value, c.options);
      return trackOrg(request, response);
    } catch {
      const response = NextResponse.redirect(new URL("/login", request.url));
      for (const name of [ACCESS_COOKIE, REFRESH_COOKIE]) response.cookies.delete(name);
      return response;
    }
  }

  return trackOrg(request, NextResponse.next());
}

export const config = {
  matcher: ["/((?!_next/static|_next/image|favicon.ico).*)"],
};
