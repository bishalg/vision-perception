"use client";

import Link from "next/link";
import { useEffect, useRef, useState, type FormEvent } from "react";
import { failureText, type ActionFailure, type ActionResult } from "@/components/exports/types";
import { EXPORT_PURPOSES, type ExportMode } from "@/lib/api/exports";
import { formatDateTime } from "@/lib/format";
import { useText } from "@/lib/i18n/provider";
import { exportsErrorMessage } from "@/messages/exports";
import { orgPaths } from "@/lib/paths";

type SessionsResult = { ok: true; sessions: { id: string; startedAt: string }[] } | ActionFailure;

export interface ShareCaseDialogProps {
  /** `export.identified` for the signed-in role: hides the identified mode only; the backend decides. */
  canIdentified: boolean;
  create: (form: FormData) => Promise<ActionResult<{ id: string }>>;
  loadSessions: () => Promise<SessionsResult>;
  /** `/{org}` (see `orgBase`): the created export opens at `data.export(id)` under it. */
  orgPath: string;
}

/**
 * "Share case" (SHR-10): requests one `case_package` export of this participant. The console holds no consent or
 * format logic (UI-2); the one check made here is the backend's own `case-consent-missing` rule, refused before the
 * request with the API's message so a clinician is not sent round-trip for an empty field.
 */
export function ShareCaseDialog({ canIdentified, create, loadSessions, orgPath }: ShareCaseDialogProps) {
  const text = useText();
  const t = text.exports.shareCase;
  const [open, setOpen] = useState(false);
  const [mode, setMode] = useState<ExportMode>("deidentified");
  const [pick, setPick] = useState(false);
  const [sessions, setSessions] = useState<SessionsResult | null>(null);
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [createdId, setCreatedId] = useState<string | null>(null);
  const first = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (open) first.current?.focus();
  }, [open]);

  useEffect(() => {
    if (!open || !pick || sessions) return;
    let live = true;
    loadSessions()
      .then((r) => live && setSessions(r))
      .catch(() => live && setSessions({ ok: false, slug: null, status: 0 }));
    return () => {
      live = false;
    };
  }, [open, pick, sessions, loadSessions]);

  function close() {
    setOpen(false);
    setError(null);
    setPick(false);
    setMode("deidentified");
  }

  async function onSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const data = new FormData(event.currentTarget);
    if (mode === "identified" && (!String(data.get("consent_reference") ?? "").trim() || !String(data.get("consent_recorded_at") ?? ""))) {
      setError(exportsErrorMessage("case-consent-missing", 400));
      return;
    }
    setPending(true);
    setError(null);
    let r: ActionResult<{ id: string }>;
    try {
      r = await create(data);
    } catch {
      r = { ok: false, slug: null, status: 0 };
    }
    setPending(false);
    if (r.ok) {
      setCreatedId(r.id);
      close();
    } else {
      setError(`${t.failed} ${failureText(r)}`);
    }
  }

  return (
    <div className="no-print stack">
      <div>
        <button type="button" onClick={() => { setCreatedId(null); setOpen(true); }}>
          {t.open}
        </button>
      </div>
      {createdId ? (
        <p role="status">
          {t.created} <Link href={orgPaths(orgPath).data.export(createdId)}>{t.view}</Link>
        </p>
      ) : null}
      {open ? (
        <div className="dialog-backdrop">
          <div
            role="dialog"
            aria-modal="true"
            aria-labelledby="share-case-title"
            className="dialog panel"
            onKeyDown={(e) => {
              if (e.key === "Escape" && !pending) close();
            }}
          >
            <h2 id="share-case-title">{t.title}</h2>
            <p className="muted">{t.intro}</p>
            <form onSubmit={onSubmit} autoComplete="off" className="stack">
              <fieldset>
                <legend>{t.mode}</legend>
                <label className="check">
                  <input
                    ref={first}
                    type="radio"
                    name="mode"
                    value="deidentified"
                    checked={mode === "deidentified"}
                    onChange={() => setMode("deidentified")}
                  />
                  {text.domain.enumLabel(text.locale, "export_mode", "deidentified")}
                </label>
                {canIdentified ? (
                  <label className="check">
                    <input
                      type="radio"
                      name="mode"
                      value="identified"
                      checked={mode === "identified"}
                      onChange={() => setMode("identified")}
                    />
                    {text.domain.enumLabel(text.locale, "export_mode", "identified")}
                  </label>
                ) : null}
                <p className="muted">{t.modeHint}</p>
              </fieldset>
              <label>
                {t.purpose}
                <select name="purpose" defaultValue="clinical_care">
                  {EXPORT_PURPOSES.map((p) => (
                    <option key={p} value={p}>{text.domain.enumLabel(text.locale, "export_purpose", p)}</option>
                  ))}
                </select>
              </label>
              <p className="muted">{t.purposeHint}</p>
              <label>
                {t.recipient}
                <input name="recipient" minLength={10} maxLength={200} required />
              </label>
              <p className="muted">{t.recipientHint}</p>
              {mode === "identified" ? (
                <>
                  <label>
                    {t.consentReference}
                    <input name="consent_reference" maxLength={200} />
                  </label>
                  <label>
                    {t.consentDate}
                    <input name="consent_recorded_at" type="date" />
                  </label>
                  <p className="muted">{t.consentHint}</p>
                  <label className="check">
                    <input name="include_attachments" type="checkbox" value="true" />
                    {t.attachments}
                  </label>
                </>
              ) : null}
              <fieldset>
                <legend>{t.sessions}</legend>
                <label className="check">
                  <input type="radio" name="sessions_scope" value="all" checked={!pick} onChange={() => setPick(false)} />
                  {t.sessionsAll}
                </label>
                <label className="check">
                  <input type="radio" name="sessions_scope" value="pick" checked={pick} onChange={() => setPick(true)} />
                  {t.sessionsPick}
                </label>
                {pick ? <SessionPicker result={sessions} /> : null}
              </fieldset>
              <label>
                {t.reason}
                <textarea name="reason" minLength={10} maxLength={500} required rows={2} />
              </label>
              <p><strong>{t.noRecall}</strong></p>
              {error ? <p role="alert" className="error">{error}</p> : null}
              <div className="dialog-actions">
                <button type="submit" disabled={pending}>{pending ? t.submitting : t.submit}</button>
                <button type="button" className="secondary" onClick={close} disabled={pending}>{t.cancel}</button>
              </div>
            </form>
          </div>
        </div>
      ) : null}
    </div>
  );
}

function SessionPicker({ result }: { result: SessionsResult | null }) {
  const t = useText().exports.shareCase;
  if (!result) return <p className="muted">{t.sessionsLoading}</p>;
  if (!result.ok) return <p role="alert" className="error">{t.sessionsFailed}</p>;
  if (result.sessions.length === 0) return <p className="muted">{t.sessionsNone}</p>;
  return (
    <div className="stack">
      {result.sessions.map((s) => (
        <label key={s.id} className="check">
          <input type="checkbox" name="session_ids" value={s.id} />
          {formatDateTime(s.startedAt)}
        </label>
      ))}
    </div>
  );
}
