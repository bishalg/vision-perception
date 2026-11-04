import { NextResponse, type NextRequest } from "next/server";
import { isOrgId } from "@/lib/orgs";

/**
 * A same-origin redirect with a relative `Location`. A route handler's `request.url` can carry another host than the
 * browser used (`next dev --hostname 127.0.0.1` reports `localhost`; a reverse proxy, its upstream address), and CSP
 * `form-action 'self'` then blocks the redirect of this GET form, leaving the user on the picker.
 */
function redirectTo(path: string): NextResponse {
  return new NextResponse(null, { status: 307, headers: { Location: path } });
}

/** Target of the org switcher's GET form: `org_id` (superadmin free entry) wins over the `org` select. */
export function GET(request: NextRequest) {
  const sp = request.nextUrl.searchParams;
  const typed = sp.get("org_id")?.trim();
  const target = typed ? typed : sp.get("org");
  if (!isOrgId(target)) return redirectTo("/?invalid=1");
  return redirectTo(`/${target.toLowerCase()}/overview`);
}
