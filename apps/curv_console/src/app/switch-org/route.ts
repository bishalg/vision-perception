import { NextResponse, type NextRequest } from "next/server";
import { isOrgId } from "@/lib/orgs";

/** Target of the org switcher's GET form: `org_id` (superadmin free entry) wins over the `org` select. */
export function GET(request: NextRequest) {
  const sp = request.nextUrl.searchParams;
  const typed = sp.get("org_id")?.trim();
  const target = typed ? typed : sp.get("org");
  if (!isOrgId(target)) return NextResponse.redirect(new URL("/?invalid=1", request.url));
  return NextResponse.redirect(new URL(`/${target.toLowerCase()}/overview`, request.url));
}
