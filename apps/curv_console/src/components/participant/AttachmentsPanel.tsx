"use client";

import { useState, useTransition } from "react";
import type { Attachment } from "@/lib/api/emr";
import { formatDateTime } from "@/lib/format";
import { formatFileSize, participantText } from "@/messages/participant";
import { AttachmentUpload, type UploadActions } from "./AttachmentUpload";
import { failureText, type ActionResult } from "./types";

const t = participantText.attachments;
const c = t.cols;

function RemoveButton({ name, remove }: { name: string; remove: () => Promise<ActionResult> }) {
  const [confirming, setConfirming] = useState(false);
  const [busy, start] = useTransition();
  const [result, setResult] = useState<ActionResult | null>(null);
  if (!confirming) {
    return (
      <button type="button" className="link-button" aria-label={t.removeLabel(name)} onClick={() => setConfirming(true)}>
        {t.remove}
      </button>
    );
  }
  return (
    <span role="alertdialog" aria-label={t.removeLabel(name)} className="confirm">
      <span>{t.confirmRemove(name)}</span>
      <button
        type="button"
        disabled={busy}
        onClick={() => start(async () => setResult(await remove().catch(() => ({ ok: false, slug: null, status: 0 }) as const)))}
      >
        {t.confirmRemoveButton}
      </button>
      <button type="button" className="secondary" disabled={busy} onClick={() => setConfirming(false)}>{t.cancel}</button>
      {result && !result.ok ? <span role="alert" className="error">{failureText(result)}</span> : null}
    </span>
  );
}

/**
 * Files tab (REC-6, REC-7). The list is `GET /v1/participants/{id}/attachments` (metadata: kind, name, size, visit);
 * opening a file is a plain link to the console's `/api/attachments/{id}` route handler, which streams it with
 * `no-store` (and the backend audits `attachment.read`). Download links only for `clinical.content` roles, and only
 * once the bytes have arrived (`sha256` set); upload for `attachment.write`.
 */
export function AttachmentsPanel({
  rows,
  canRead,
  canWrite,
  actions,
  visits,
}: {
  rows: Attachment[];
  canRead: boolean;
  canWrite: boolean;
  actions: UploadActions;
  visits: { id: string; label: string }[];
}) {
  const visitLabel = new Map(visits.map((v) => [v.id, v.label]));
  return (
    <section aria-labelledby="files-h" className="stack">
      <h2 id="files-h">{t.title}</h2>
      <p className="muted">{t.intro}</p>
      {canWrite ? <AttachmentUpload actions={actions} visits={visits} /> : <p className="muted">{t.readOnly}</p>}
      {!canRead ? <p className="muted">{t.noDownload}</p> : null}
      {rows.length === 0 ? (
        <p className="muted">{t.empty}</p>
      ) : (
        <div className="table-wrap">
          <table className="data responsive" aria-label={t.title}>
            <thead>
              <tr>
                <th scope="col">{c.name}</th>
                <th scope="col">{c.kind}</th>
                <th scope="col">{c.size}</th>
                <th scope="col">{c.when}</th>
                <th scope="col">{c.visit}</th>
                {canRead || canWrite ? <th scope="col">{c.actions}</th> : null}
              </tr>
            </thead>
            <tbody>
              {rows.map((r) => {
                const name = r.file_name || t.unnamed;
                return (
                  <tr key={r.id} data-testid="attachment-row">
                    <td data-label={c.name}>{name}</td>
                    <td data-label={c.kind}>{t.kinds[r.kind]}</td>
                    <td data-label={c.size}>{formatFileSize(r.byte_size)}</td>
                    <td data-label={c.when}>{formatDateTime(r.created_at)}</td>
                    <td data-label={c.visit}>{(r.encounter_id && visitLabel.get(r.encounter_id)) || "-"}</td>
                    {canRead || canWrite ? (
                      <td data-label={c.actions}>
                        {!r.sha256 ? (
                          <span className="muted">{t.incomplete}</span>
                        ) : canRead ? (
                          <a href={`/api/attachments/${encodeURIComponent(r.id)}`} download aria-label={t.downloadLabel(name)}>
                            {t.download}
                          </a>
                        ) : null}{" "}
                        {canWrite ? <RemoveButton name={name} remove={() => actions.remove(r.id)} /> : null}
                      </td>
                    ) : null}
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      )}
    </section>
  );
}
