/**
 * Public share-link download (SHR-9, WP-C.19): `GET /v1/share/{link_id}/file?t=<download_token>`, streamed back by
 * `app/api/share/[id]/route.ts`. Lives with the API client so all backend calls stay in lib/api, like `exportFile.ts`.
 * No bearer is sent: the download token is the credential. Neither the token nor the backend URL is ever logged or
 * returned to the browser.
 */
import { ApiError, PROBLEM_BASE, problemSchema } from "./types";
import { apiMode, getApi } from "./index";

export type ShareDownload =
  | { kind: "stream"; response: Response }
  /** `slug` is the problem slug when the backend sent one (`share-link-expired`, `share-link-exhausted`, ...). */
  | { kind: "error"; status: number; slug: string | null };

/** Statuses the recipient may learn about; everything else is reported as 502. */
const PASS_THROUGH = [401, 403, 404, 410, 429];

/** Download tokens are opaque; this only rejects what cannot be one before it reaches a URL. */
export function plausibleDownloadToken(token: string): boolean {
  return token.length > 0 && token.length <= 2048 && /^[A-Za-z0-9._~+/=-]+$/.test(token);
}

async function slugOf(res: Response): Promise<string | null> {
  try {
    const parsed = problemSchema.safeParse(await res.json());
    if (parsed.success && parsed.data.type.startsWith(PROBLEM_BASE)) return parsed.data.type.slice(PROBLEM_BASE.length);
  } catch {
    // not problem+json
  }
  return null;
}

export async function resolveShareDownload(
  linkId: string,
  downloadToken: string,
  env: Record<string, string | undefined> = process.env,
  fetchImpl: typeof fetch = fetch,
): Promise<ShareDownload> {
  if (!plausibleDownloadToken(downloadToken)) return { kind: "error", status: 401, slug: null };
  let upstream: Response;
  try {
    if (apiMode(env) === "mock") {
      upstream = await getApi(undefined, env).mockShareFile!(linkId, downloadToken);
    } else {
      const base = (env.CURV_API_BASE_URL ?? "").replace(/\/+$/, "");
      const url = `${base}/v1/share/${encodeURIComponent(linkId)}/file?t=${encodeURIComponent(downloadToken)}`;
      upstream = await fetchImpl(url, { cache: "no-store", redirect: "error" });
    }
  } catch (err) {
    if (err instanceof ApiError) {
      return { kind: "error", status: PASS_THROUGH.includes(err.status) ? err.status : 502, slug: err.slug };
    }
    return { kind: "error", status: 502, slug: null };
  }
  if (!upstream.ok || !upstream.body) {
    const status = PASS_THROUGH.includes(upstream.status) ? upstream.status : 502;
    return { kind: "error", status, slug: status === 502 ? null : await slugOf(upstream) };
  }
  return { kind: "stream", response: upstream };
}
