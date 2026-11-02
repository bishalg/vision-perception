"use server";

import { ApiError, getApi } from "@/lib/api";
import { shareErrorKey } from "@/messages/exports";

export type ShareOpenResult =
  | { ok: true; fileName: string; bytes: number; expiresAt: string; downloadsLeft: number; downloadToken: string }
  | { ok: false; error: string; retryAfter: number | null };

/** Typing aids only (the alphabet has no lower case, `I`, `O`, `0` or `1`): spaces and dashes go, letters are upper-cased. */
function normalisePasscode(raw: string): string {
  return raw.replace(/[\s-]+/g, "").toUpperCase();
}

/**
 * The passcode step of the public share page (SHR-9, UI-3 rules): the passcode arrives once in the POST body, is sent
 * once to `POST /v1/share/{id}/open` on a client built without any access token (the route is public, and a signed-in
 * clinician's token must not ride along), and is neither returned, stored, logged nor put in a URL. Only what the
 * recipient needs to download comes back.
 */
export async function openShareAction(linkId: string, formData: FormData): Promise<ShareOpenResult> {
  const passcode = normalisePasscode(String(formData.get("passcode") ?? ""));
  if (!passcode || passcode.length > 64) return { ok: false, error: "share-passcode-invalid", retryAfter: null };
  try {
    const opened = await getApi().openShareLink(linkId, passcode);
    return {
      ok: true,
      fileName: opened.file_name,
      bytes: opened.bytes,
      expiresAt: opened.expires_at,
      downloadsLeft: opened.downloads_left,
      downloadToken: opened.download_token,
    };
  } catch (err) {
    if (err instanceof ApiError) {
      return { ok: false, error: shareErrorKey(err.slug, err.status), retryAfter: err.status === 429 ? err.retryAfter : null };
    }
    return { ok: false, error: "fallback", retryAfter: null };
  }
}
