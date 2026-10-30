/** Binary report access (GET /v1/sessions/{id}/report.pdf). Lives with the API client so all backend calls stay in lib/api. */
import { apiMode } from "./index";

// Minimal valid one-page PDF used in mock mode.
const MOCK_PDF = `%PDF-1.1
1 0 obj<</Type/Catalog/Pages 2 0 R>>endobj
2 0 obj<</Type/Pages/Kids[3 0 R]/Count 1>>endobj
3 0 obj<</Type/Page/Parent 2 0 R/MediaBox[0 0 300 100]>>endobj
trailer<</Root 1 0 R>>
%%EOF
`;

export async function fetchReportPdf(
  sessionId: string,
  accessToken: string,
  env: Record<string, string | undefined> = process.env,
  fetchImpl: typeof fetch = fetch,
): Promise<Response> {
  if (apiMode(env) === "mock") {
    return new Response(MOCK_PDF, { status: 200, headers: { "Content-Type": "application/pdf" } });
  }
  const base = (env.CURV_API_BASE_URL ?? "").replace(/\/+$/, "");
  return fetchImpl(`${base}/v1/sessions/${encodeURIComponent(sessionId)}/report.pdf`, {
    headers: { Authorization: `Bearer ${accessToken}`, Accept: "application/pdf" },
    cache: "no-store",
  });
}
