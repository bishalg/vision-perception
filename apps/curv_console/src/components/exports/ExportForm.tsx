"use client";

import Link from "next/link";
import { useActionState } from "react";
import { EXPORT_FORMATS, EXPORT_MODES, EXPORT_PURPOSES, type Integration } from "@/lib/api/exports";
import { ATTENTION_FLAGS, QUALITY_VERDICTS } from "@/lib/api/types";
import { CLINICAL_STATES } from "@/lib/api/clinical";
import { exportsText } from "@/messages/exports";
import { failureText, type ActionResult } from "./types";

const t = exportsText.create;

type State = ActionResult<{ id: string }> | null;

/**
 * Request an export. Which purpose/mode pairs are allowed, and what the role may include, is decided by the
 * backend (it answers 400 consent-purpose-mismatch or 403); this form only collects the request. The identified
 * mode is offered only to roles that hold the permission.
 */
export function ExportForm({
  create,
  canCreate,
  canIdentified,
  destinations,
  orgId,
}: {
  create: (formData: FormData) => Promise<ActionResult<{ id: string }>>;
  canCreate: boolean;
  canIdentified: boolean;
  /** Active destinations the caller may list (admins and owners); empty hides the chooser. */
  destinations: Integration[];
  orgId: string;
}) {
  const [state, action, pending] = useActionState<State, FormData>((_prev, fd) => create(fd), null);
  if (!canCreate) return <p className="muted">{t.readOnly}</p>;
  const modes = EXPORT_MODES.filter((m) => m !== "identified" || canIdentified);
  return (
    <form action={action} className="stack panel" aria-label={t.title}>
      <label>
        {t.format}
        <select name="format" defaultValue="csv">
          {EXPORT_FORMATS.filter((f) => f !== "pdf_summary").map((f) => (
            <option key={f} value={f}>{exportsText.formats[f]}</option>
          ))}
        </select>
      </label>
      <label>
        {t.purpose}
        <select name="purpose" defaultValue="research">
          {EXPORT_PURPOSES.map((p) => (
            <option key={p} value={p}>{exportsText.purposes[p]}</option>
          ))}
        </select>
      </label>
      <label>
        {t.mode}
        <select name="mode" defaultValue="deidentified">
          {modes.map((m) => (
            <option key={m} value={m}>{exportsText.modes[m]}</option>
          ))}
        </select>
      </label>
      <p className="muted">{t.purposeHint}</p>
      <label>
        {t.reason}
        <textarea name="reason" rows={3} minLength={10} maxLength={500} required />
      </label>
      <fieldset className="stack">
        <legend>{t.filters}</legend>
        <label>
          {t.from}
          <input name="from" type="date" />
        </label>
        <label>
          {t.to}
          <input name="to" type="date" />
        </label>
        <label>
          {t.flag}
          <select name="flag" defaultValue="">
            <option value="">{t.anyOption}</option>
            {ATTENTION_FLAGS.map((f) => (
              <option key={f} value={f}>{t.flags[f]}</option>
            ))}
          </select>
        </label>
        <label>
          {t.quality}
          <select name="quality" defaultValue="">
            <option value="">{t.anyOption}</option>
            {QUALITY_VERDICTS.map((q) => (
              <option key={q} value={q}>{t.qualities[q]}</option>
            ))}
          </select>
        </label>
        <label>
          {t.reviewed}
          <select name="reviewed" defaultValue="">
            <option value="">{t.reviewedAny}</option>
            <option value="yes">{t.reviewedYes}</option>
            <option value="no">{t.reviewedNo}</option>
          </select>
        </label>
        <fieldset>
          <legend>{t.clinicalState}</legend>
          {["none", ...CLINICAL_STATES].map((s) => (
            <label key={s} className="check">
              <input type="checkbox" name="clinical_state" value={s} />
              {t.clinicalStates[s] ?? s}
            </label>
          ))}
        </fieldset>
        <label>
          {t.bundleVersion}
          <input name="bundle_version" maxLength={64} autoComplete="off" />
        </label>
        <label>
          {t.sessionIds}
          <textarea name="session_ids" rows={2} autoComplete="off" />
        </label>
        <label>
          {t.participantIds}
          <textarea name="participant_ids" rows={2} autoComplete="off" />
        </label>
      </fieldset>
      {destinations.length > 0 ? (
        <fieldset>
          <legend>{t.destinations}</legend>
          {destinations.map((d) => (
            <label key={d.id} className="check">
              <input type="checkbox" name="destination_ids" value={d.id} />
              {d.name}
            </label>
          ))}
          <p className="muted">{t.destinationsHint}</p>
        </fieldset>
      ) : null}
      <div>
        <button type="submit" disabled={pending}>{t.submit}</button>
      </div>
      {state?.ok ? (
        <p role="status">
          {t.queued}{" "}
          <Link href={`/${encodeURIComponent(orgId)}/exports/${encodeURIComponent(state.id)}`}>{t.view}</Link>
        </p>
      ) : null}
      {state && !state.ok ? (
        <p role="alert" className="error">{t.failed} {failureText(state)}</p>
      ) : null}
    </form>
  );
}
