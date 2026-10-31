"use client";

import { useEffect, useRef, useState, type FormEvent } from "react";
import type { ClinicalReview } from "@/lib/api/clinical";
import { clinicalErrorMessage, clinicalText } from "@/messages/clinical";
import type { ReviewResult } from "./types";

const t = clinicalText;

export function AmendDialog({
  submit,
  onDone,
  onClose,
}: {
  submit: (reason: string) => Promise<ReviewResult>;
  onDone: (review: ClinicalReview) => void;
  onClose: () => void;
}) {
  const [reason, setReason] = useState("");
  const [pending, setPending] = useState(false);
  const [failure, setFailure] = useState<{ slug: string | null; status: number } | null>(null);
  const field = useRef<HTMLTextAreaElement>(null);
  useEffect(() => field.current?.focus(), []);
  const length = reason.trim().length;

  async function onSubmit(event: FormEvent) {
    event.preventDefault();
    setPending(true);
    setFailure(null);
    let result: ReviewResult;
    try {
      result = await submit(reason.trim());
    } catch {
      result = { ok: false, slug: null, status: 0 };
    }
    setPending(false);
    if (result.ok) onDone(result.review);
    else setFailure({ slug: result.slug, status: result.status });
  }

  return (
    <div className="dialog-backdrop">
      <div
        role="dialog"
        aria-modal="true"
        aria-labelledby="amend-title"
        className="dialog panel"
        onKeyDown={(e) => {
          if (e.key === "Escape") onClose();
        }}
      >
        <h2 id="amend-title">{t.dialog.amendTitle}</h2>
        <p className="muted">{t.dialog.amendIntro}</p>
        <form onSubmit={onSubmit} className="stack">
          <label>
            {t.dialog.amendReason}
            <textarea ref={field} rows={4} value={reason} maxLength={500} onChange={(e) => setReason(e.target.value)} />
          </label>
          <p className="muted">{t.review.limit(length, 500)}</p>
          {failure ? (
            <p role="alert" className="error">
              {clinicalErrorMessage(failure.slug, failure.status)}
            </p>
          ) : null}
          <div className="dialog-actions">
            <button type="submit" disabled={pending || length < 10}>
              {t.dialog.confirmAmend}
            </button>
            <button type="button" className="secondary" onClick={onClose} disabled={pending}>
              {t.dialog.cancel}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}
