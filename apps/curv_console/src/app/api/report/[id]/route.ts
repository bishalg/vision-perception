import { NextResponse } from "next/server";
import { fetchReportPdf } from "@/lib/api/report";
import { accessToken } from "@/lib/auth/server";

export async function GET(_req: Request, ctx: { params: Promise<{ id: string }> }) {
  const token = await accessToken();
  if (!token) return new NextResponse("Unauthorized", { status: 401 });
  const { id } = await ctx.params;
  const upstream = await fetchReportPdf(id, token);
  // The server renders the report (WP-B.8): 404 = unknown session, 409 = simulated or
  // synthetic capture (SIM-5); 401/403 pass through so the browser can re-authenticate.
  if (!upstream.ok || !upstream.body) {
    const status = [401, 403, 404, 409].includes(upstream.status) ? upstream.status : 502;
    return new NextResponse("Report unavailable", { status });
  }
  return new NextResponse(upstream.body, {
    headers: {
      "Content-Type": "application/pdf",
      "Content-Disposition": `attachment; filename="report-${id.replace(/[^A-Za-z0-9_-]/g, "")}.pdf"`,
      "Cache-Control": "no-store",
    },
  });
}
