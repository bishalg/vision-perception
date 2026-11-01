/**
 * Export download (POST /v1/exports/{id}/download-url, then the signed URL). Lives with the API client so all
 * backend calls stay in lib/api. The signed URL is never sent to the browser as data: a presigned (absolute) URL
 * becomes a 303 redirect, and a proxy-mode (relative) URL is fetched here and streamed back.
 */
import { ApiError } from "./types";
import { apiMode, getApi } from "./index";

const MOCK_FILE = "participant_token,flag\nTEST-0000,green\n";

export type ExportDownload =
  | { kind: "redirect"; location: string }
  | { kind: "stream"; response: Response }
  | { kind: "error"; status: number };

/** Statuses the browser may see; everything else is reported as 502. */
const PASS_THROUGH = [401, 403, 404, 409, 410];

export async function resolveExportDownload(
  exportId: string,
  accessToken: string,
  env: Record<string, string | undefined> = process.env,
  fetchImpl: typeof fetch = fetch,
): Promise<ExportDownload> {
  let url: string;
  try {
    url = (await getApi(accessToken, env).createExportDownloadUrl(exportId)).url;
  } catch (err) {
    const status = err instanceof ApiError && PASS_THROUGH.includes(err.status) ? err.status : 502;
    return { kind: "error", status };
  }
  if (/^https?:\/\//i.test(url)) return { kind: "redirect", location: url };
  if (apiMode(env) === "mock") {
    return {
      kind: "stream",
      response: new Response(MOCK_FILE, {
        status: 200,
        headers: { "Content-Type": "text/csv", "Content-Disposition": 'attachment; filename="export-mock.csv"' },
      }),
    };
  }
  // Proxy blob mode: a path on the backend whose `t` token is the credential. Only /v1/exports/ paths are followed.
  if (!/^\/v1\/exports\/[^/?#]+\/file\?t=[^&#]+$/.test(url)) return { kind: "error", status: 502 };
  const base = (env.CURV_API_BASE_URL ?? "").replace(/\/+$/, "");
  const upstream = await fetchImpl(`${base}${url}`, { cache: "no-store" });
  if (!upstream.ok || !upstream.body) {
    return { kind: "error", status: PASS_THROUGH.includes(upstream.status) ? upstream.status : 502 };
  }
  return { kind: "stream", response: upstream };
}
