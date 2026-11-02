import { NextResponse } from "next/server";
import { resolveShareDownload } from "@/lib/api/shareFile";
import { shareErrorKey } from "@/messages/exports";

/** HIP-9: never cached, never indexed (next.config.ts sets the same; repeated so no response can miss them). */
const PRIVATE = { "Cache-Control": "no-store", "X-Robots-Tag": "noindex, nofollow" } as const;

/**
 * Public download of a shared case package (SHR-9, WP-C.19). POST only, from the `/share/[id]` page's form: the
 * five-minute download token from the passcode step travels in the form body, never in a URL the browser sees or
 * logs. No session is read or set; the token is the credential and is never logged. The file is streamed from the
 * backend (UI-2: the console holds no file and no format logic). A refusal sends the recipient back to the page
 * with an error key only (`?e=`), which carries nothing about the link beyond the backend's answer.
 */
export async function POST(req: Request, ctx: { params: Promise<{ id: string }> }) {
  const { id } = await ctx.params;
  const page = `/share/${encodeURIComponent(id)}`;
  let token = "";
  try {
    const form = await req.formData();
    token = String(form.get("t") ?? "");
  } catch {
    token = "";
  }
  const result = await resolveShareDownload(id, token);
  if (result.kind === "error") {
    const key = shareErrorKey(result.slug, result.status);
    return new NextResponse(null, { status: 303, headers: { Location: `${page}?e=${encodeURIComponent(key)}`, ...PRIVATE } });
  }
  const { response } = result;
  const headers = new Headers({
    "Content-Type": response.headers.get("content-type") ?? "application/octet-stream",
    "X-Content-Type-Options": "nosniff",
    ...PRIVATE,
  });
  const disposition = response.headers.get("content-disposition");
  headers.set("Content-Disposition", disposition?.startsWith("attachment") ? disposition : "attachment");
  const length = response.headers.get("content-length");
  if (length) headers.set("Content-Length", length);
  return new NextResponse(response.body, { status: 200, headers });
}
