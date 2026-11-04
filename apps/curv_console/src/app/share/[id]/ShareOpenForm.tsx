"use client";

import { useState, type FormEvent } from "react";
import { formatDateTime } from "@/lib/format";
import { messagesFor } from "@/lib/i18n/messages";
import type { Locale } from "@/lib/i18n/types";
import type { ShareOpenResult } from "./actions";

type Opened = Extract<ShareOpenResult, { ok: true }>;

function size(bytes: number): string {
  if (bytes >= 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  if (bytes >= 1024) return `${Math.round(bytes / 1024)} KB`;
  return `${bytes} B`;
}

/**
 * Passcode step, then the download button (SHR-10). The passcode input is uncontrolled: it is read into a `FormData`
 * at submit and the form is reset before the server answers, so it never enters React state, storage or a URL. The
 * download token comes back for the download form's body only (POST to `/api/share/[id]`), and is dropped from the
 * page once the download has started.
 */
export function ShareOpenForm({
  linkId,
  locale,
  open,
  initialError,
}: {
  linkId: string;
  /** From the page's `Accept-Language` negotiation (never the signed-in user's setting: the page is public). */
  locale: Locale;
  open: (form: FormData) => Promise<ShareOpenResult>;
  initialError: string | null;
}) {
  const t = messagesFor(locale).exports.sharePage;
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<{ key: string; retryAfter: number | null } | null>(
    initialError ? { key: initialError, retryAfter: null } : null,
  );
  const [opened, setOpened] = useState<Opened | null>(null);
  const [started, setStarted] = useState(false);

  async function onSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const form = event.currentTarget;
    const data = new FormData(form);
    form.reset(); // the passcode leaves the DOM now, not when the server answers
    setPending(true);
    setError(null);
    let result: ShareOpenResult;
    try {
      result = await open(data);
    } catch {
      result = { ok: false, error: "fallback", retryAfter: null };
    }
    setPending(false);
    if (result.ok) {
      setStarted(false);
      setOpened(result);
    } else {
      setError({ key: result.error, retryAfter: result.retryAfter });
    }
  }

  if (opened && !started) {
    return (
      <section className="stack" aria-label={t.download}>
        <dl className="facts">
          <div>
            <dt>{t.file}</dt>
            <dd data-testid="share-file">{opened.fileName}</dd>
          </div>
          <div>
            <dt>{t.size}</dt>
            <dd>{size(opened.bytes)}</dd>
          </div>
          <div>
            <dt>{t.expires}</dt>
            <dd>{formatDateTime(opened.expiresAt)}</dd>
          </div>
          <div>
            <dt>{t.downloadsLeft}</dt>
            <dd data-testid="share-downloads-left">{opened.downloadsLeft}</dd>
          </div>
        </dl>
        <form
          method="post"
          action={`/api/share/${encodeURIComponent(linkId)}`}
          onSubmit={() => {
            // Let the browser take the form data first, then drop the token from the page.
            window.setTimeout(() => {
              setStarted(true);
              setOpened(null);
            }, 200);
          }}
        >
          <input type="hidden" name="t" value={opened.downloadToken} />
          <button type="submit">{t.download}</button>
        </form>
        <p className="muted">{t.downloadHint}</p>
      </section>
    );
  }

  return (
    <div className="stack">
      {started ? <p role="status">{t.started}</p> : null}
      <form onSubmit={onSubmit} autoComplete="off" className="stack">
        <label>
          {t.passcode}
          <input
            name="passcode"
            type="text"
            // Plain text, not a password field, so browsers do not offer to save it.
            autoComplete="off"
            autoCapitalize="characters"
            spellCheck={false}
            maxLength={64}
            required
          />
        </label>
        <p className="muted">{t.passcodeHint}</p>
        {error ? (
          <div role="alert" className="error">
            <p>{t.errors[error.key] ?? t.errors.fallback}</p>
            {error.retryAfter != null ? <p>{t.retryAfter(error.retryAfter)}</p> : null}
          </div>
        ) : null}
        <button type="submit" disabled={pending}>
          {pending ? t.opening : t.open}
        </button>
      </form>
    </div>
  );
}
