"use client";

import { useEffect, useRef, useState, type FormEvent } from "react";
import {
  ENCOUNTER_KINDS,
  MAX_ENCOUNTER_REASON_CHARS,
  MAX_ENCOUNTER_SUMMARY_CHARS,
  type Encounter,
} from "@/lib/api/emr";
import { useText } from "@/lib/i18n/provider";
import { failureText, type ActionResult } from "./types";

export interface SessionChoice {
  id: string;
  label: string;
}

/** `YYYY-MM-DD` of an instant in the browser's time zone (what a date picker shows). */
export function localDay(at: Date | string): string {
  const d = typeof at === "string" ? new Date(at) : at;
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
}

/**
 * Visit date -> instant: today means "now"; another day means midday local time, so the day is the same in any zone a
 * reader is likely in. The follow-up date means the end of that local day (due "by" the date).
 */
export function visitInstant(day: string, now: Date = new Date()): string | null {
  if (!/^\d{4}-\d{2}-\d{2}$/.test(day)) return null;
  if (day === localDay(now)) return now.toISOString();
  const d = new Date(`${day}T12:00:00`);
  return Number.isNaN(d.getTime()) ? null : d.toISOString();
}
export function dueInstant(day: string): string | null {
  if (!/^\d{4}-\d{2}-\d{2}$/.test(day)) return null;
  const d = new Date(`${day}T23:59:00`);
  return Number.isNaN(d.getTime()) ? null : d.toISOString();
}

/**
 * New visit or (with `encounter`) a change to one. Plain choices first (type, date), free text last and short. The
 * summary field exists only for roles that may write it (`canSummary`); for the others nothing is sent, so the API
 * never sees an empty summary from them. Text goes to the server action only: never a URL or the page title.
 */
export function EncounterForm({
  submit,
  sessions,
  canSummary,
  encounter,
  onDone,
}: {
  submit: (form: FormData) => Promise<ActionResult>;
  sessions: SessionChoice[];
  canSummary: boolean;
  encounter?: Encounter;
  onDone: () => void;
}) {
  const text = useText();
  const t = text.participant.visits;
  const [pending, setPending] = useState(false);
  const [failure, setFailure] = useState<ActionResult | null>(null);
  const first = useRef<HTMLSelectElement>(null);
  useEffect(() => first.current?.focus(), []);
  // Rendered only after a click, so reading the browser clock here cannot cause a hydration mismatch.
  const [today] = useState(() => localDay(new Date()));

  async function onSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const data = new FormData(event.currentTarget);
    const occurred = visitInstant(String(data.get("date") ?? ""));
    data.delete("date");
    if (occurred) data.set("occurred_at", occurred);
    const due = dueInstant(String(data.get("follow_up_date") ?? ""));
    data.delete("follow_up_date");
    if (due) data.set("follow_up_due_at", due);
    setPending(true);
    setFailure(null);
    let result: ActionResult;
    try {
      result = await submit(data);
    } catch {
      result = { ok: false, slug: null, status: 0 };
    }
    setPending(false);
    if (result.ok) onDone();
    else setFailure(result);
  }

  const id = encounter ? `visit-${encounter.id}` : "visit-new";
  return (
    <form onSubmit={onSubmit} className="stack card" aria-label={encounter ? t.editTitle : t.addTitle}>
      <h3>{encounter ? t.editTitle : t.addTitle}</h3>
      <label>
        {t.kind}
        <select ref={first} name="kind" required defaultValue={encounter?.kind ?? "follow_up_visit"}>
          {ENCOUNTER_KINDS.map((k) => (
            <option key={k} value={k}>{text.domain.enumLabel(text.locale, "encounter_kind", k)}</option>
          ))}
        </select>
      </label>
      <label>
        {t.date}
        <input type="date" name="date" required max={today} defaultValue={encounter ? localDay(encounter.occurred_at) : today} />
      </label>
      <label htmlFor={`${id}-reason`}>{t.reason}</label>
      <input id={`${id}-reason`} name="reason" maxLength={MAX_ENCOUNTER_REASON_CHARS} autoComplete="off" defaultValue={encounter?.reason ?? ""} />
      <p className="muted">{t.reasonHelp(MAX_ENCOUNTER_REASON_CHARS)}</p>
      {canSummary ? (
        <>
          <label htmlFor={`${id}-summary`}>{t.summary}</label>
          <textarea id={`${id}-summary`} name="summary" rows={5} maxLength={MAX_ENCOUNTER_SUMMARY_CHARS} defaultValue={encounter?.summary ?? ""} />
          <p className="muted">{t.summaryHelp(MAX_ENCOUNTER_SUMMARY_CHARS)}</p>
        </>
      ) : (
        <p className="muted">{t.summaryNoWrite}</p>
      )}
      <label>
        {t.followUp}
        <input
          type="date"
          name="follow_up_date"
          min={encounter ? undefined : today}
          defaultValue={encounter?.follow_up_due_at ? localDay(encounter.follow_up_due_at) : ""}
        />
      </label>
      <p className="muted">{t.followUpHelp}</p>
      {!encounter ? (
        <fieldset>
          <legend>{t.sessions}</legend>
          {sessions.length === 0 ? (
            <p className="muted">{t.noSessions}</p>
          ) : (
            sessions.map((s) => (
              <label key={s.id} className="check">
                <input type="checkbox" name="session_ids" value={s.id} /> {s.label}
              </label>
            ))
          )}
        </fieldset>
      ) : null}
      {failure && !failure.ok ? (
        <p role="alert" className="error">{t.failed} {failureText(failure, text.locale)}</p>
      ) : null}
      <div className="dialog-actions">
        <button type="submit" disabled={pending}>{pending ? t.saving : t.save}</button>
        <button type="button" className="secondary" onClick={onDone} disabled={pending}>{t.cancel}</button>
      </div>
    </form>
  );
}
