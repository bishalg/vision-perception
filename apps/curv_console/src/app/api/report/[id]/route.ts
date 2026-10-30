import { NextResponse } from "next/server";
import { fetchReportPdf } from "@/lib/api/report";
import { accessToken } from "@/lib/auth/server";

export async function GET(_req: Request, ctx: { params: Promise<{ id: string }> }) {
  const token = await accessToken();
  if (!token) return new NextResponse("Unauthorized", { status: 401 });
  const { id } = await ctx.params;
  const upstream = await fetchReportPdf(id, token);
  if (!upstream.ok || !upstream.body) {
    return new NextResponse("Report unavailable", { status: upstream.status === 404 ? 404 : 502 });
  }
  return new NextResponse(upstream.body, {
    headers: {
      "Content-Type": "application/pdf",
      "Content-Disposition": `attachment; filename="report-${id.replace(/[^A-Za-z0-9_-]/g, "")}.pdf"`,
      "Cache-Control": "no-store",
    },
  });
}
