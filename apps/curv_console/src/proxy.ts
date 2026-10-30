import { NextResponse, type NextRequest } from "next/server";
import { getApi } from "@/lib/api";
import { ACCESS_COOKIE, REFRESH_COOKIE, cookieSecure, sessionCookies } from "@/lib/auth/cookies";
import { decideGuard } from "@/lib/auth/guard";

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
      return response;
    } catch {
      const response = NextResponse.redirect(new URL("/login", request.url));
      for (const name of [ACCESS_COOKIE, REFRESH_COOKIE]) response.cookies.delete(name);
      return response;
    }
  }

  return NextResponse.next();
}

export const config = {
  matcher: ["/((?!_next/static|_next/image|favicon.ico).*)"],
};
