/**
 * Share-a-case links (SHR-8, SHR-9, WP-C.19): create, list and revoke the share links of a case-package export, and
 * the public open of `/share/[id]`. The zod schemas mirror `asd_contract::emr::{ShareLink, CreateShareLinkRequest,
 * CreatedShareLink, ShareLinkPage, ShareLinkOpenRequest, ShareLinkOpenResponse}` in
 * `docs/spec/schemas/asd/api-v1.json` (`share.contract.test.ts` guards the drift).
 *
 * The passcode is returned once by `createShareLink` and sent once by `openShareLink`; the console never stores,
 * logs or puts it in a URL. The file itself is streamed by `shareFile.ts` (`GET /v1/share/{id}/file?t=`), never
 * through this JSON client.
 */
import { z } from "@/lib/zod";
import type { LiveRequest } from "./clinical";

const optStr = z.string().nullable().optional();
const optCount = z.number().int().min(0).optional();

/** One share link as listed. Never carries the passcode. `url` is `/share/<id>`, absolute when the backend knows the console origin. */
export const shareLinkSchema = z.object({
  id: z.string(),
  export_id: z.string(),
  url: z.string(),
  expires_at: z.string(),
  max_downloads: z.number().int(),
  created_at: z.string(),
  downloads: optCount,
  opens: optCount,
  revoked_at: optStr,
  last_opened_at: optStr,
});
export type ShareLink = z.infer<typeof shareLinkSchema>;

/** `POST /v1/exports/{id}/share-link`. Omitted fields take the backend defaults (72 h, one download). */
export const createShareLinkRequestSchema = z.object({
  expires_in_days: z.number().int().min(0).nullable().optional(),
  max_downloads: z.number().int().nullable().optional(),
});
export type CreateShareLinkRequest = z.infer<typeof createShareLinkRequestSchema>;

/** The `201` answer: the passcode is here this once and never again. */
export const createdShareLinkSchema = z.object({ link: shareLinkSchema, passcode: z.string() });
export type CreatedShareLink = z.infer<typeof createdShareLinkSchema>;

export const shareLinkPageSchema = z.object({
  items: z.array(shareLinkSchema),
  next_cursor: z.string().nullable().optional(),
});
export type ShareLinkPage = z.infer<typeof shareLinkPageSchema>;

export const shareLinkOpenRequestSchema = z.object({ passcode: z.string() });
export type ShareLinkOpenRequest = z.infer<typeof shareLinkOpenRequestSchema>;

/** `200` answer to the public open: what the recipient is about to download, and a five-minute download token. */
export const shareLinkOpenResponseSchema = z.object({
  export_id: z.string(),
  file_name: z.string(),
  bytes: z.number().int().min(0),
  expires_at: z.string(),
  downloads_left: z.number().int(),
  download_token: z.string(),
});
export type ShareLinkOpenResponse = z.infer<typeof shareLinkOpenResponseSchema>;

/**
 * Expiry and download choices offered by the console. They are the contract's ranges (`CreateShareLinkRequest`:
 * 1..7 days, default 3; 1..5 downloads, default 1, `asd_contract::emr::SHARE_LINK_*`), checked against the
 * descriptions in api-v1.json by `share.contract.test.ts`; the backend validates the values it receives.
 */
export const SHARE_LINK_DAY_CHOICES = [1, 2, 3, 5, 7] as const;
export const SHARE_LINK_DEFAULT_DAYS = 3;
export const SHARE_LINK_DOWNLOAD_CHOICES = [1, 2, 3, 4, 5] as const;
export const SHARE_LINK_DEFAULT_DOWNLOADS = 1;

/** Display state of a link, derived from the fields the backend returns (the backend decides what it serves). */
export type ShareLinkState = "active" | "revoked" | "expired" | "exhausted";
export function shareLinkState(link: ShareLink, now: number): ShareLinkState {
  if (link.revoked_at) return "revoked";
  if (Date.parse(link.expires_at) <= now) return "expired";
  if ((link.downloads ?? 0) >= link.max_downloads) return "exhausted";
  return "active";
}

export interface ShareLinkListQuery {
  cursor?: string;
  limit?: number;
}

export interface ShareApi {
  /** `POST /v1/exports/{id}/share-link` (`ShareLinkManage`; the export must be a ready `case_package`). */
  createShareLink(exportId: string, request?: CreateShareLinkRequest): Promise<CreatedShareLink>;
  /** `GET /v1/exports/{id}/share-links`. */
  listShareLinks(exportId: string, query?: ShareLinkListQuery): Promise<ShareLinkPage>;
  /** `DELETE /v1/share-links/{id}` (revoke; `204`). */
  revokeShareLink(linkId: string): Promise<void>;
  /**
   * Public `POST /v1/share/{id}/open {passcode}`. Call it on a client built without an access token: the route takes
   * no bearer. `404` unknown or revoked, `410 share-link-expired | share-link-exhausted | export-expired`,
   * `401 share-passcode-invalid`, `429` with `Retry-After`.
   */
  openShareLink(linkId: string, passcode: string): Promise<ShareLinkOpenResponse>;
  /**
   * Mock mode only: the public `GET /v1/share/{id}/file?t=` against the in-memory backend (counts the download).
   * The live client does not implement it; `shareFile.ts` streams the backend's response instead.
   */
  mockShareFile?(linkId: string, downloadToken: string): Promise<Response>;
}

export function createLiveShare(request: LiveRequest, buildQuery: (q: object) => string): ShareApi {
  const enc = encodeURIComponent;
  return {
    createShareLink: (exportId, body) =>
      request(`/exports/${enc(exportId)}/share-link`, createdShareLinkSchema, { method: "POST", body: body ?? {} }),
    listShareLinks: (exportId, q) =>
      request(`/exports/${enc(exportId)}/share-links${buildQuery(q ?? {})}`, shareLinkPageSchema),
    revokeShareLink: (linkId) => request(`/share-links/${enc(linkId)}`, null, { method: "DELETE" }),
    openShareLink: (linkId, passcode) =>
      request(`/share/${enc(linkId)}/open`, shareLinkOpenResponseSchema, { method: "POST", body: { passcode } }),
  };
}
