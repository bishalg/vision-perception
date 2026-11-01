"use client";

import Link from "next/link";
import { useActionState, useState } from "react";
import { privacyText } from "@/messages/privacy";
import { privacyFailureText, type PrivacyResult } from "./types";

const t = privacyText.participantAction;

type State = PrivacyResult<{ duplicate: boolean }> | null;

/**
 * "Request erasure" on one participant row. The confirmation is in the page, not a browser dialog. The backend
 * answers 200 `duplicate` when an open request exists; that is shown as a note, not as a new request.
 */
export function ErasureAction({
  token,
  request,
  requestsHref,
}: {
  token: string;
  request: () => Promise<PrivacyResult<{ duplicate: boolean }>>;
  requestsHref: string;
}) {
  const [confirming, setConfirming] = useState(false);
  const [state, action, pending] = useActionState<State, FormData>(async () => {
    const result = await request();
    setConfirming(false);
    return result;
  }, null);
  return (
    <div className="stack">
      {!confirming ? (
        <div>
          <button type="button" className="danger" onClick={() => setConfirming(true)} aria-label={`${t.open} ${token}`}>
            {t.open}
          </button>
        </div>
      ) : (
        <form action={action} className="stack">
          <p role="alertdialog" aria-label={t.title}>{t.confirm(token)}</p>
          <div className="filter-actions">
            <button type="submit" className="danger" disabled={pending}>{t.confirmButton}</button>
            <button type="button" onClick={() => setConfirming(false)} disabled={pending}>{t.cancel}</button>
          </div>
        </form>
      )}
      {state?.ok ? (
        <p role="status">
          {state.duplicate ? t.duplicate : t.queued} <Link href={requestsHref}>{t.viewRequests}</Link>
        </p>
      ) : null}
      {state && !state.ok ? <p role="alert" className="error">{t.failed} {privacyFailureText(state)}</p> : null}
    </div>
  );
}
