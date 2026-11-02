/**
 * Attachment bytes (REC-7): `PUT /v1/attachments/{id}/content` and `GET /v1/attachments/{id}`, used only by the
 * `app/api/attachments/...` route handlers. Lives with the API client so every backend call stays in lib/api (as
 * `report.ts` and `exportFile.ts`). The console never inspects, hashes or stores the bytes: the browser computes the
 * `Content-Digest`, the backend verifies it, encrypts and stores; here they are only forwarded.
 */
import { ApiError, problemSchema } from "./types";
import { apiMode, getApi } from "./index";
import { MAX_ATTACHMENT_BYTES, attachmentMediaTypeSchema, type AttachmentMediaType } from "./emr";

/** Mock-only hooks the mock client carries next to `EmrApi` (never part of the live interface). */
export interface MockAttachmentContent {
  mockPutAttachmentContent(attachmentId: string, bytes: Uint8Array, mediaType: string, digest: string | null): Promise<void>;
  mockGetAttachmentContent(attachmentId: string): Promise<{ bytes: Uint8Array; media_type: AttachmentMediaType; file_name: string }>;
}

/** Outcome for the browser: a problem slug and status only, never the upstream detail text. */
export type AttachmentFailure = { kind: "error"; status: number; slug: string | null };

const PASS_THROUGH = [400, 401, 403, 404, 409, 412, 413, 415, 429];

function failure(status: number, slug: string | null): AttachmentFailure {
  return { kind: "error", status: PASS_THROUGH.includes(status) ? status : 502, slug: PASS_THROUGH.includes(status) ? slug : null };
}

async function slugOf(res: Response): Promise<string | null> {
  try {
    const parsed = problemSchema.safeParse(await res.json());
    if (!parsed.success) return null;
    return new ApiError(res.status, parsed.data).slug;
  } catch {
    return null;
  }
}

function fromError(err: unknown): AttachmentFailure {
  if (err instanceof ApiError) return failure(err.status, err.slug);
  return failure(502, null);
}

const base = (env: Record<string, string | undefined>) => (env.CURV_API_BASE_URL ?? "").replace(/\/+$/, "");

/**
 * Forward one upload. `bytes` is the request body as received (the route handler already refused anything over
 * `MAX_ATTACHMENT_BYTES`); `digest` is the browser's `Content-Digest` header, passed through unchanged.
 */
export async function uploadAttachmentContent(
  attachmentId: string,
  bytes: Uint8Array,
  mediaType: string,
  digest: string | null,
  accessToken: string,
  env: Record<string, string | undefined> = process.env,
  fetchImpl: typeof fetch = fetch,
): Promise<{ kind: "ok" } | AttachmentFailure> {
  if (bytes.byteLength > MAX_ATTACHMENT_BYTES) return failure(400, "attachment-too-large");
  if (apiMode(env) === "mock") {
    try {
      await (getApi(accessToken, env) as unknown as MockAttachmentContent).mockPutAttachmentContent(attachmentId, bytes, mediaType, digest);
      return { kind: "ok" };
    } catch (err) {
      return fromError(err);
    }
  }
  const headers: Record<string, string> = {
    Authorization: `Bearer ${accessToken}`,
    Accept: "application/json",
    "Content-Type": mediaType,
  };
  if (digest) headers["Content-Digest"] = digest;
  const res = await fetchImpl(`${base(env)}/v1/attachments/${encodeURIComponent(attachmentId)}/content`, {
    method: "PUT",
    headers,
    body: bytes as BodyInit,
    cache: "no-store",
  });
  if (res.ok) return { kind: "ok" };
  return failure(res.status, await slugOf(res));
}

export type AttachmentDownload =
  | { kind: "stream"; body: ReadableStream<Uint8Array> | Uint8Array; mediaType: string; disposition: string }
  | AttachmentFailure;

/** File names are sanitised by the server to `[A-Za-z0-9._-]`; this keeps the header safe whatever arrives. */
function disposition(fileName: string | null, id: string): string {
  const safe = (fileName ?? "").replace(/[^A-Za-z0-9._-]/g, "_").replace(/^\.+/, "").slice(0, 120);
  return `attachment; filename="${safe || `attachment-${id.replace(/[^A-Za-z0-9_-]/g, "")}`}"`;
}

/** `GET /v1/attachments/{id}`: decrypted by the backend (`ClinicalContentRead`, audited `attachment.read`). */
export async function downloadAttachment(
  attachmentId: string,
  accessToken: string,
  env: Record<string, string | undefined> = process.env,
  fetchImpl: typeof fetch = fetch,
): Promise<AttachmentDownload> {
  if (apiMode(env) === "mock") {
    try {
      const file = await (getApi(accessToken, env) as unknown as MockAttachmentContent).mockGetAttachmentContent(attachmentId);
      return { kind: "stream", body: file.bytes, mediaType: file.media_type, disposition: disposition(file.file_name, attachmentId) };
    } catch (err) {
      return fromError(err);
    }
  }
  const res = await fetchImpl(`${base(env)}/v1/attachments/${encodeURIComponent(attachmentId)}`, {
    headers: { Authorization: `Bearer ${accessToken}` },
    cache: "no-store",
  });
  if (!res.ok || !res.body) return failure(res.ok ? 502 : res.status, res.ok ? null : await slugOf(res));
  const type = attachmentMediaTypeSchema.safeParse((res.headers.get("content-type") ?? "").split(";")[0]!.trim());
  const upstream = res.headers.get("content-disposition");
  const name = upstream?.match(/filename="?([^";]+)"?/)?.[1] ?? null;
  return {
    kind: "stream",
    body: res.body,
    mediaType: type.success ? type.data : "application/octet-stream",
    disposition: disposition(name, attachmentId),
  };
}
