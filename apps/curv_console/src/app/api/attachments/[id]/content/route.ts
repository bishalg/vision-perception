import { NextResponse } from "next/server";
import { MAX_ATTACHMENT_BYTES } from "@/lib/api/emr";
import { uploadAttachmentContent } from "@/lib/api/attachmentFile";
import { accessToken } from "@/lib/auth/server";

const NO_STORE = { "Cache-Control": "no-store" };

function refuse(status: number, slug: string | null) {
  return NextResponse.json({ slug }, { status, headers: NO_STORE });
}

/**
 * Upload one attachment's bytes (REC-7). Server actions cap bodies at 1 MB, so the browser `PUT`s the file here and
 * this forwards it, with the browser's `Content-Digest` (RFC 9530, verified by the backend over these exact bytes),
 * to `PUT /v1/attachments/{id}/content`. Anything over 10 MiB is refused before it is read. The proxy buffers request
 * bodies up to Next's default 10 MB (`proxyClientMaxBodySize`), which equals the limit.
 */
export async function PUT(req: Request, ctx: { params: Promise<{ id: string }> }) {
  const token = await accessToken();
  if (!token) return refuse(401, "unauthorized");
  const declared = Number(req.headers.get("content-length") ?? "");
  if (Number.isFinite(declared) && declared > MAX_ATTACHMENT_BYTES) return refuse(400, "attachment-too-large");
  const bytes = new Uint8Array(await req.arrayBuffer());
  if (bytes.byteLength > MAX_ATTACHMENT_BYTES) return refuse(400, "attachment-too-large");
  if (bytes.byteLength === 0) return refuse(400, "validation-failed");
  const mediaType = (req.headers.get("content-type") ?? "").split(";")[0]!.trim();
  const { id } = await ctx.params;
  const result = await uploadAttachmentContent(id, bytes, mediaType, req.headers.get("content-digest"), token);
  if (result.kind === "error") return refuse(result.status, result.slug);
  return new NextResponse(null, { status: 204, headers: NO_STORE });
}
