"use client";

import { useRef, useState, type FormEvent } from "react";
import { ATTACHMENT_KINDS, ATTACHMENT_MEDIA_TYPES, MAX_ATTACHMENT_BYTES } from "@/lib/api/emr";
import { participantErrorMessage, participantText } from "@/messages/participant";
import { failureText, type ActionResult } from "./types";

const t = participantText.attachments;

export interface AttachmentMetaInput {
  kind: string;
  file_name: string;
  media_type: string;
  byte_size: number;
  encounter_id?: string;
}

export interface UploadActions {
  /** Server action: metadata row first; answers the attachment id. */
  create: (meta: AttachmentMetaInput) => Promise<ActionResult<{ id: string }>>;
  /** Server action: drop the metadata row when the bytes did not arrive. */
  remove: (attachmentId: string) => Promise<ActionResult>;
  /** Server action: revalidate the workspace after a complete upload. */
  uploaded: () => Promise<void>;
}

/**
 * RFC 9530 `Content-Digest` of the exact bytes sent (`sha-256=:<base64>:`, with the colons). `null` where the browser
 * has no SubtleCrypto (a plain-http origin): the header is optional and the backend still stores the hash it computes.
 */
export async function contentDigest(data: Blob | ArrayBuffer): Promise<string | null> {
  const subtle = globalThis.crypto?.subtle;
  if (!subtle) return null;
  const buf = data instanceof Blob ? await data.arrayBuffer() : data;
  const hash = new Uint8Array(await subtle.digest("SHA-256", buf));
  let bin = "";
  for (const b of hash) bin += String.fromCharCode(b);
  return `sha-256=:${btoa(bin)}:`;
}

/** Client-side refusals use the same words as the API's `attachment-too-large` / `attachment-type-not-allowed`. */
export function checkFile(file: { size: number; type: string } | null): string | null {
  if (!file || file.size === 0) return t.empty_file;
  if (file.size > MAX_ATTACHMENT_BYTES) return participantErrorMessage("attachment-too-large", 400);
  if (!(ATTACHMENT_MEDIA_TYPES as readonly string[]).includes(file.type)) return participantErrorMessage("attachment-type-not-allowed", 400);
  return null;
}

/**
 * Upload form (REC-7). Three steps, one button: metadata through a server action, then the bytes with `PUT` to the
 * console's `/api/attachments/{id}/content` route handler (server actions cap bodies at 1 MB), which forwards them and
 * the `Content-Digest` to the backend. A failed byte upload removes the metadata row again.
 */
export function AttachmentUpload({
  actions,
  visits,
}: {
  actions: UploadActions;
  /** Visits to link the file to (id, label). */
  visits: { id: string; label: string }[];
}) {
  const form = useRef<HTMLFormElement>(null);
  const fileInput = useRef<HTMLInputElement>(null);
  const [pending, setPending] = useState(false);
  const [message, setMessage] = useState<{ ok: boolean; text: string } | null>(null);

  async function onSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const data = new FormData(event.currentTarget);
    // The picked file itself (not a FormData copy): its bytes are what the digest covers and what is sent.
    const picked = fileInput.current?.files?.[0] ?? null;
    const refusal = checkFile(picked);
    if (refusal || !picked) {
      setMessage({ ok: false, text: refusal ?? t.empty_file });
      return;
    }
    setPending(true);
    setMessage(null);
    try {
      const digest = await contentDigest(picked);
      const visit = String(data.get("encounter_id") ?? "");
      const created = await actions.create({
        kind: String(data.get("kind") ?? ""),
        file_name: picked.name,
        media_type: picked.type,
        byte_size: picked.size,
        ...(visit ? { encounter_id: visit } : {}),
      });
      if (!created.ok) {
        setMessage({ ok: false, text: `${t.failed} ${failureText(created)}` });
        return;
      }
      const res = await fetch(`/api/attachments/${encodeURIComponent(created.id)}/content`, {
        method: "PUT",
        headers: { "Content-Type": picked.type, ...(digest ? { "Content-Digest": digest } : {}) },
        body: picked,
        cache: "no-store",
      });
      if (!res.ok) {
        const body = (await res.json().catch(() => null)) as { slug?: string | null } | null;
        await actions.remove(created.id).catch(() => undefined);
        setMessage({ ok: false, text: `${t.failed} ${participantErrorMessage(body?.slug ?? null, res.status)}` });
        return;
      }
      form.current?.reset();
      setMessage({ ok: true, text: t.uploaded });
      await actions.uploaded();
    } catch {
      setMessage({ ok: false, text: `${t.failed} ${participantErrorMessage(null, 0)}` });
    } finally {
      setPending(false);
    }
  }

  return (
    <form ref={form} onSubmit={onSubmit} className="stack card" aria-label={t.uploadTitle}>
      <h3>{t.uploadTitle}</h3>
      <label>
        {t.file}
        {/* Checked in onSubmit (empty, size, type) so the message is ours, in the same words as the API. */}
        <input ref={fileInput} type="file" name="file" aria-required="true" accept={ATTACHMENT_MEDIA_TYPES.join(",")} />
      </label>
      <label>
        {t.kind}
        <select name="kind" defaultValue="paper_form" required>
          {ATTACHMENT_KINDS.map((k) => (
            <option key={k} value={k}>{t.kinds[k]}</option>
          ))}
        </select>
      </label>
      {visits.length > 0 ? (
        <label>
          {t.visit}
          <select name="encounter_id" defaultValue="">
            <option value="">{t.noVisit}</option>
            {visits.map((v) => (
              <option key={v.id} value={v.id}>{v.label}</option>
            ))}
          </select>
        </label>
      ) : null}
      <div>
        <button type="submit" disabled={pending}>{pending ? t.uploading : t.upload}</button>
      </div>
      {message ? (
        <p role={message.ok ? "status" : "alert"} className={message.ok ? undefined : "error"}>{message.text}</p>
      ) : null}
    </form>
  );
}
