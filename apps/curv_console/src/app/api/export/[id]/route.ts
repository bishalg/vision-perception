import { NextResponse } from "next/server";
import { resolveExportDownload } from "@/lib/api/exportFile";
import { accessToken } from "@/lib/auth/server";

/**
 * Download an export artifact. POST only: every download is audited by the backend, so a plain link or a prefetch
 * must not trigger one. The signed URL stays on the server (redirect or stream, never page data).
 */
export async function POST(_req: Request, ctx: { params: Promise<{ id: string }> }) {
  const token = await accessToken();
  if (!token) return new NextResponse("Unauthorized", { status: 401 });
  const { id } = await ctx.params;
  const result = await resolveExportDownload(id, token);
  if (result.kind === "redirect") {
    return new NextResponse(null, { status: 303, headers: { Location: result.location, "Cache-Control": "no-store" } });
  }
  if (result.kind === "error") {
    const text: Record<number, string> = {
      401: "Unauthorized",
      403: "Not allowed to download this export",
      404: "Export not found",
      409: "The export is not ready to download",
      410: "The export has expired; request a new one",
    };
    return new NextResponse(text[result.status] ?? "Export unavailable", { status: result.status });
  }
  const { response } = result;
  const headers = new Headers({
    "Content-Type": response.headers.get("content-type") ?? "application/octet-stream",
    "Cache-Control": "no-store",
    "X-Content-Type-Options": "nosniff",
  });
  const disposition = response.headers.get("content-disposition");
  if (disposition) headers.set("Content-Disposition", disposition);
  return new NextResponse(response.body, { status: 200, headers });
}
