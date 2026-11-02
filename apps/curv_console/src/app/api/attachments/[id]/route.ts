import { NextResponse } from "next/server";
import { downloadAttachment } from "@/lib/api/attachmentFile";
import { accessToken } from "@/lib/auth/server";

const NO_STORE = { "Cache-Control": "no-store", "X-Content-Type-Options": "nosniff" };

/**
 * Open one attachment (REC-7): the backend decrypts it (`ClinicalContentRead`) and audits `attachment.read`; this
 * streams it back with `no-store` and as a download (never rendered inline under the console origin). The token stays
 * on the server; errors carry the problem slug only.
 */
export async function GET(_req: Request, ctx: { params: Promise<{ id: string }> }) {
  const token = await accessToken();
  if (!token) return NextResponse.json({ slug: "unauthorized" }, { status: 401, headers: NO_STORE });
  const { id } = await ctx.params;
  const result = await downloadAttachment(id, token);
  if (result.kind === "error") return NextResponse.json({ slug: result.slug }, { status: result.status, headers: NO_STORE });
  return new NextResponse(result.body as BodyInit, {
    status: 200,
    headers: { ...NO_STORE, "Content-Type": result.mediaType, "Content-Disposition": result.disposition },
  });
}
