"use client";

import { useEffect, useRef, useState, type FormEvent } from "react";
import type { ClinicalReview } from "@/lib/api/clinical";
import { clinicalErrorMessage, clinicalText, hashPrefix } from "@/messages/clinical";
import type { ReviewResult } from "./types";

const t = clinicalText;

export interface StepUpDialogProps {
  kind: "sign" | "cosign";
  review: ClinicalReview;
  /** Server action wrapper. Receives the form data once; this component keeps no copy of it. */
  submit: (formData: FormData) => Promise<ReviewResult>;
  onDone: (review: ClinicalReview) => void;
  onClose: () => void;
  /** Called when the user accepts the "reload" prompt after content-changed. */
  onReload: () => void;
}

/**
 * Step-up signing dialog (UI-3). The password and TOTP inputs are uncontrolled: they live only in the DOM, are read
 * into a `FormData` at submit, and the form is reset before the request is awaited, so neither value ever enters
 * React state. The content hash shown is the API's value and is sent back unchanged.
 */
export function StepUpDialog({ kind, review, submit, onDone, onClose, onReload }: StepUpDialogProps) {
  const [pending, setPending] = useState(false);
  const [failure, setFailure] = useState<{ slug: string | null; status: number } | null>(null);
  const first = useRef<HTMLInputElement>(null);
  useEffect(() => first.current?.focus(), []);

  async function onSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const form = event.currentTarget;
    const data = new FormData(form);
    data.set("content_hash", review.content_hash);
    form.reset(); // the secret leaves the DOM now, not when the server answers
    setPending(true);
    setFailure(null);
    let result: ReviewResult;
    try {
      result = await submit(data);
    } catch {
      result = { ok: false, slug: null, status: 0 };
    }
    setPending(false);
    if (result.ok) onDone(result.review);
    else setFailure({ slug: result.slug, status: result.status });
  }

  const title = kind === "sign" ? t.dialog.signTitle : t.dialog.cosignTitle;
  const mustReload = failure?.slug != null && t.reloadSlugs.includes(failure.slug);
  return (
    <div className="dialog-backdrop">
      <div
        role="dialog"
        aria-modal="true"
        aria-labelledby="stepup-title"
        className="dialog panel"
        onKeyDown={(e) => {
          if (e.key === "Escape") onClose();
        }}
      >
        <h2 id="stepup-title">{title}</h2>
        <blockquote className="meaning">{t.dialog.meaning}</blockquote>
        <p>
          {t.dialog.hashLabel}: <code data-testid="hash-prefix">{hashPrefix(review.content_hash)}</code>
        </p>
        <p className="muted">{t.dialog.hashNote}</p>
        <form onSubmit={onSubmit} autoComplete="off" className="stack">
          <label>
            {t.dialog.password}
            <input ref={first} name="password" type="password" autoComplete="current-password" required />
          </label>
          <label>
            {t.dialog.totp}
            <input name="totp_code" inputMode="numeric" autoComplete="one-time-code" pattern="[0-9]*" maxLength={8} />
          </label>
          <label className="check">
            <input name="meaning_ack" type="checkbox" value="true" required />
            {t.dialog.ack}
          </label>
          <p className="muted">{t.dialog.secretNote}</p>
          {failure ? (
            <div role="alert" className="error">
              <p>{clinicalErrorMessage(failure.slug, failure.status)}</p>
              {mustReload ? (
                <button type="button" onClick={onReload}>
                  {t.review.reload}
                </button>
              ) : null}
            </div>
          ) : null}
          <div className="dialog-actions">
            <button type="submit" disabled={pending}>
              {pending ? t.dialog.submitting : kind === "sign" ? t.dialog.confirmSign : t.dialog.confirmCosign}
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

export function SignDialog(props: Omit<StepUpDialogProps, "kind">) {
  return <StepUpDialog kind="sign" {...props} />;
}
