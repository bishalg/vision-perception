"use client";

import { useActionState, useRef, useState, useTransition } from "react";
import { MAX_ASSIGNING_AUTHORITY_CHARS, MAX_MRN_CHARS } from "@/lib/api/participant";
import { formatDateTime } from "@/lib/format";
import { participantText } from "@/messages/participant";
import { failureText, type ActionResult, type RevealResult } from "./types";

const t = participantText.mrn;

type Form = (form: FormData) => Promise<ActionResult>;

export interface MrnPanelProps {
  /** `none` = the org disabled MRNs (MRN-2); `unknown` = the org settings were not readable, so the backend decides. */
  mode: "mrn" | "none" | "unknown";
  /** From the participant detail: absent for roles without `PatientRefRead`. */
  hasRef: boolean | null | undefined;
  canRead: boolean;
  canWrite: boolean;
  reveal: () => Promise<RevealResult>;
  save: Form;
  unlink: () => Promise<ActionResult>;
  search: (form: FormData) => Promise<ActionResult<{ found: boolean }>>;
}

/**
 * MRN panel (MRN-1..6, UI-4). The MRN is shown only after an explicit "Show MRN" (each view is audited by the backend),
 * and typed or searched only in POST forms handled by server actions: it never reaches a URL, a title or the console log.
 */
export function MrnPanel(p: MrnPanelProps) {
  const [shown, setShown] = useState<{ authority: string; mrn: string; updatedAt: string } | null>(null);
  const [revealError, setRevealError] = useState<string | null>(null);
  const [editing, setEditing] = useState(false);
  const [confirming, setConfirming] = useState(false);
  const [busy, startTransition] = useTransition();
  const [unlinkState, setUnlinkState] = useState<ActionResult | null>(null);
  const saveRef = useRef<HTMLFormElement>(null);
  const searchRef = useRef<HTMLFormElement>(null);

  const [saveState, saveAction, saving] = useActionState<ActionResult | null, FormData>(async (_s, form) => {
    const r = await p.save(form);
    if (r.ok) {
      saveRef.current?.reset();
      setShown(null);
      setEditing(false);
    }
    return r;
  }, null);
  const [searchState, searchAction, searching] = useActionState<ActionResult<{ found: boolean }> | null, FormData>(
    async (_s, form) => {
      const r = await p.search(form);
      searchRef.current?.reset();
      return r;
    },
    null,
  );

  if (p.mode === "none") {
    return (
      <section aria-labelledby="mrn-h" className="stack">
        <h2 id="mrn-h">{t.title}</h2>
        <p role="status" data-testid="mrn-disabled">
          <strong>{t.disabledTitle}.</strong> {t.disabled}
        </p>
      </section>
    );
  }
  if (!p.canRead) {
    return (
      <section aria-labelledby="mrn-h" className="stack">
        <h2 id="mrn-h">{t.title}</h2>
        <p className="muted">{t.noRead}</p>
      </section>
    );
  }

  const revealNow = () =>
    startTransition(async () => {
      setRevealError(null);
      const r = await p.reveal();
      if (r.ok) setShown({ authority: r.authority, mrn: r.mrn, updatedAt: r.updatedAt });
      else setRevealError(failureText(r));
    });
  const unlinkNow = () =>
    startTransition(async () => {
      const r = await p.unlink();
      setUnlinkState(r);
      if (r.ok) {
        setShown(null);
        setConfirming(false);
      }
    });

  const showForm = p.canWrite && (!p.hasRef || editing);
  return (
    <section aria-labelledby="mrn-h" className="stack">
      <h2 id="mrn-h">{t.title}</h2>
      <p className="muted">{t.intro}</p>

      {p.hasRef ? (
        <div className="card stack" data-testid="mrn-on-file">
          <p>{t.onFile}</p>
          {shown ? (
            <dl className="facts">
              <div>
                <dt>{t.authority}</dt>
                <dd>{shown.authority}</dd>
              </div>
              <div>
                <dt>{t.number}</dt>
                <dd data-testid="mrn-value">{shown.mrn}</dd>
              </div>
              <div>
                <dt>{t.updated}</dt>
                <dd>{formatDateTime(shown.updatedAt)}</dd>
              </div>
            </dl>
          ) : null}
          <div className="filter-actions">
            {shown ? (
              <button type="button" onClick={() => setShown(null)}>{t.hide}</button>
            ) : (
              <button type="button" onClick={revealNow} disabled={busy}>{t.reveal}</button>
            )}
            {p.canWrite && !editing ? (
              <button type="button" onClick={() => setEditing(true)}>{t.replaceTitle}</button>
            ) : null}
            {p.canWrite && !confirming ? (
              <button type="button" className="danger" onClick={() => setConfirming(true)}>{t.unlink}</button>
            ) : null}
          </div>
          {confirming ? (
            <div className="stack">
              <p role="alertdialog" aria-label={t.unlink}>{t.confirmUnlink}</p>
              <div className="filter-actions">
                <button type="button" className="danger" onClick={unlinkNow} disabled={busy}>{t.confirmUnlinkButton}</button>
                <button type="button" onClick={() => setConfirming(false)} disabled={busy}>{t.cancel}</button>
              </div>
            </div>
          ) : null}
          {revealError ? <p role="alert" className="error">{t.failed} {revealError}</p> : null}
          {unlinkState && !unlinkState.ok ? <p role="alert" className="error">{t.failed} {failureText(unlinkState)}</p> : null}
        </div>
      ) : (
        <p data-testid="mrn-none">{t.none}</p>
      )}
      {unlinkState?.ok && !p.hasRef ? <p role="status">{t.unlinked}</p> : null}

      {showForm ? (
        <form ref={saveRef} action={saveAction} className="card stack" aria-label={p.hasRef ? t.replaceTitle : t.linkTitle}>
          <h3>{p.hasRef ? t.replaceTitle : t.linkTitle}</h3>
          <label>
            {t.authority}
            <input name="assigning_authority" required maxLength={MAX_ASSIGNING_AUTHORITY_CHARS} autoComplete="off" />
          </label>
          <p className="muted">{t.authorityHelp}</p>
          <label>
            {t.number}
            <input name="mrn" required maxLength={MAX_MRN_CHARS} autoComplete="off" />
          </label>
          <div className="filter-actions">
            <button type="submit" disabled={saving}>{saving ? t.saving : t.save}</button>
            {editing ? <button type="button" onClick={() => setEditing(false)} disabled={saving}>{t.cancel}</button> : null}
          </div>
          {saveState?.ok ? <p role="status">{t.saved}</p> : null}
          {saveState && !saveState.ok ? <p role="alert" className="error">{t.failed} {failureText(saveState)}</p> : null}
        </form>
      ) : null}

      <form ref={searchRef} action={searchAction}className="card stack" aria-label={t.searchTitle}>
        <h3>{t.searchTitle}</h3>
        <p className="muted">{t.searchHelp}</p>
        <label>
          {t.authority}
          <input name="assigning_authority" required maxLength={MAX_ASSIGNING_AUTHORITY_CHARS} autoComplete="off" />
        </label>
        <label>
          {t.number}
          <input name="mrn" required maxLength={MAX_MRN_CHARS} autoComplete="off" />
        </label>
        <div className="filter-actions">
          <button type="submit" disabled={searching}>{t.searchButton}</button>
        </div>
        {searchState?.ok && !searchState.found ? <p role="status">{t.notFound}</p> : null}
        {searchState && !searchState.ok ? <p role="alert" className="error">{t.failed} {failureText(searchState)}</p> : null}
      </form>
    </section>
  );
}
