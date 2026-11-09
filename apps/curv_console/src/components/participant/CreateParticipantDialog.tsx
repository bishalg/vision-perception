"use client";

import Link from "next/link";
import { useEffect, useRef, useState, type FormEvent } from "react";
import { QrCode } from "@/components/QrCode";
import { useText } from "@/lib/i18n/provider";
import { AGE_GROUPS } from "@/messages/participant";
import { ageBracketLabel, languageLabel } from "./labels";
import { failureText, type ActionResult } from "./types";
import { orgPaths } from "@/lib/paths";

/** The languages a participant can be recorded with. */
const LANGUAGES = ["en", "ne"] as const;

export interface CreatedParticipantView {
  id: string;
  participant_token: string;
  qr_payload: string;
}

/**
 * "New participant" (REC-1): one button, a short form of optional facts, then the server-minted code and its QR for
 * the clinic iPad. The code and QR stay inside this dialog; the "Open record" link carries only the participant id.
 */
export function CreateParticipantDialog({
  create,
  orgPath,
}: {
  create: (form: FormData) => Promise<ActionResult<{ participant: CreatedParticipantView }>>;
  /** `/org`, for the record link. */
  orgPath: string;
}) {
  const text = useText();
  const t = text.participant.create;
  const [open, setOpen] = useState(false);
  const [pending, setPending] = useState(false);
  const [failure, setFailure] = useState<ActionResult | null>(null);
  const [created, setCreated] = useState<CreatedParticipantView | null>(null);
  const first = useRef<HTMLSelectElement>(null);
  const done = useRef<HTMLAnchorElement>(null);
  useEffect(() => {
    if (!open) return;
    if (created) done.current?.focus();
    else first.current?.focus();
  }, [open, created]);

  function close() {
    setOpen(false);
    setCreated(null);
    setFailure(null);
  }

  async function onSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const data = new FormData(event.currentTarget);
    setPending(true);
    setFailure(null);
    let r: ActionResult<{ participant: CreatedParticipantView }>;
    try {
      r = await create(data);
    } catch {
      r = { ok: false, slug: null, status: 0 };
    }
    setPending(false);
    if (r.ok) setCreated(r.participant);
    else setFailure(r);
  }

  if (!open) {
    return (
      <button type="button" onClick={() => setOpen(true)}>
        {t.button}
      </button>
    );
  }
  return (
    <div className="dialog-backdrop">
      <div
        role="dialog"
        aria-modal="true"
        aria-labelledby="new-participant-title"
        className="dialog panel"
        onKeyDown={(e) => {
          if (e.key === "Escape" && !pending) close();
        }}
      >
        {created ? (
          <div className="stack" data-testid="created-participant">
            <h2 id="new-participant-title">{t.doneTitle}</h2>
            <p>
              {t.code}: <strong data-testid="created-token">{created.participant_token}</strong>
            </p>
            <QrCode value={created.qr_payload} label={t.qrLabel(created.participant_token)} />
            <p className="muted">{t.qrHelp}</p>
            <div className="dialog-actions no-print">
              <Link ref={done} className="button" href={orgPaths(orgPath).clinic.participant(created.id)}>
                {t.open}
              </Link>
              <button type="button" className="secondary" onClick={() => window.print()}>{t.print}</button>
              <button type="button" className="secondary" onClick={() => setCreated(null)}>{t.another}</button>
              <button type="button" className="secondary" onClick={close}>{t.close}</button>
            </div>
          </div>
        ) : (
          <form onSubmit={onSubmit} className="stack" aria-labelledby="new-participant-title">
            <h2 id="new-participant-title">{t.title}</h2>
            <p className="muted">{t.intro}</p>
            <label>
              {t.age}
              <select ref={first} name="age_bracket" defaultValue="">
                <option value="">{t.ageNone}</option>
                {AGE_GROUPS.map((a) => (
                  <option key={a} value={a}>{ageBracketLabel(text.locale, a)}</option>
                ))}
              </select>
            </label>
            <label>
              {t.language}
              <select name="language" defaultValue="">
                <option value="">{t.languageNone}</option>
                {LANGUAGES.map((code) => (
                  <option key={code} value={code}>{languageLabel(text.locale, code)}</option>
                ))}
              </select>
            </label>
            <label>
              {t.consentVersion}
              <input name="consent_version" maxLength={64} autoComplete="off" />
            </label>
            <label>
              {t.consentDate}
              <input type="date" name="consent_date" />
            </label>
            <p className="muted">{t.consentHelp}</p>
            {failure && !failure.ok ? <p role="alert" className="error">{t.failed} {failureText(failure, text.locale)}</p> : null}
            <div className="dialog-actions">
              <button type="submit" disabled={pending}>{pending ? t.creating : t.submit}</button>
              <button type="button" className="secondary" onClick={close} disabled={pending}>{t.close}</button>
            </div>
          </form>
        )}
      </div>
    </div>
  );
}
