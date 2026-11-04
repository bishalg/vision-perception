"use client";

import Link from "next/link";
import { useActionState, useState } from "react";
import {
  ANALYSIS_ENCODINGS,
  ANALYSIS_LAYOUTS,
  EXPORT_FORMATS,
  EXPORT_MODES,
  EXPORT_PURPOSES,
  isDatasetFormat,
  isDedicatedFormat,
  type DeliverableDestination,
  type ExportFormat,
} from "@/lib/api/exports";
import { ATTENTION_FLAGS, QUALITY_VERDICTS } from "@/lib/api/types";
import { CLINICAL_STATES } from "@/lib/api/clinical";
import { useText } from "@/lib/i18n/provider";
import { failureText, type ActionResult } from "./types";

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
  prefill,
}: {
  create: (formData: FormData) => Promise<ActionResult<{ id: string }>>;
  canCreate: boolean;
  canIdentified: boolean;
  /** Active destinations an exporter may pick from (`GET /orgs/{id}/export-destinations`); empty hides the chooser. */
  destinations: DeliverableDestination[];
  orgId: string;
  /** From `?format=analysis_results&run=`: the results of one analysis run (RUN-7). The format, mode and run are fixed. */
  prefill?: { runId: string };
}) {
  const all = useText();
  const text = all.exports;
  const t = text.exports.create;
  const [state, action, pending] = useActionState<State, FormData>((_prev, fd) => create(fd), null);
  const [format, setFormat] = useState<ExportFormat>("csv");
  const [layout, setLayout] = useState<(typeof ANALYSIS_LAYOUTS)[number]>("aggregates");
  if (!canCreate) return <p className="muted">{t.readOnly}</p>;
  if (prefill) {
    const a = text.analysis;
    return (
      <form action={action} className="stack panel" aria-label={t.title}>
        <input type="hidden" name="format" value="analysis_results" />
        <input type="hidden" name="mode" value="deidentified" />
        <input type="hidden" name="analysis_run_id" value={prefill.runId} />
        <p>
          {t.format}: <strong>{text.exports.formats.analysis_results}</strong>
        </p>
        <p className="muted">{a.fixedFormat}</p>
        <p>
          {a.run}: <code data-testid="prefill-run">{prefill.runId}</code>
        </p>
        <label>
          {t.purpose}
          <select name="purpose" defaultValue="research">
            {EXPORT_PURPOSES.filter((p) => p === "research" || p === "quality_improvement").map((p) => (
              <option key={p} value={p}>{text.exports.purposes[p]}</option>
            ))}
          </select>
        </label>
        <p className="muted">{a.purposeOnly}</p>
        <p className="muted">{a.deidentifiedOnly}</p>
        <label>
          {a.layout}
          <select name="analysis_layout" value={layout} onChange={(e) => setLayout(e.target.value as typeof layout)}>
            {ANALYSIS_LAYOUTS.map((l) => <option key={l} value={l}>{a.layouts[l]}</option>)}
          </select>
        </label>
        <label>
          {a.encoding}
          <select name="analysis_encoding" key={layout} defaultValue="jsonl">
            {ANALYSIS_ENCODINGS.filter((e) => e !== "parquet" || layout === "sessions").map((e) => (
              <option key={e} value={e}>{a.encodings[e]}</option>
            ))}
          </select>
        </label>
        <p className="muted">{a.parquetNote}</p>
        <label>
          {t.reason}
          <textarea name="reason" rows={3} minLength={10} maxLength={500} required />
        </label>
        <div>
          <button type="submit" disabled={pending}>{t.submit}</button>
        </div>
        {state?.ok ? (
          <p role="status">
            {t.queued}{" "}
            <Link href={`/${encodeURIComponent(orgId)}/exports/${encodeURIComponent(state.id)}`}>{t.view}</Link>
          </p>
        ) : null}
        {state && !state.ok ? <p role="alert" className="error">{t.failed} {failureText(state, text.exports.errors)}</p> : null}
      </form>
    );
  }
  // Datasets (WP-B.9) are always de-identified research exports: the identified mode and the purpose are fixed.
  const dataset = isDatasetFormat(format);
  const modes = EXPORT_MODES.filter((m) => (m !== "identified" || canIdentified) && !(dataset && m === "identified"));
  return (
    <form action={action} className="stack panel" aria-label={t.title}>
      <label>
        {t.format}
        <select name="format" value={format} onChange={(e) => setFormat(e.target.value as ExportFormat)}>
          {EXPORT_FORMATS.filter((f) => f !== "pdf_summary" && !isDedicatedFormat(f)).map((f) => (
            <option key={f} value={f}>{text.exports.formats[f]}</option>
          ))}
        </select>
      </label>
      <label>
        {t.purpose}
        <select name="purpose" key={dataset ? "ds" : "std"} defaultValue="research" disabled={dataset}>
          {EXPORT_PURPOSES.filter((p) => !dataset || p === "research").map((p) => (
            <option key={p} value={p}>{text.exports.purposes[p]}</option>
          ))}
        </select>
      </label>
      <label>
        {t.mode}
        <select name="mode" key={dataset ? "ds" : "std"} defaultValue="deidentified" disabled={dataset}>
          {modes.map((m) => (
            <option key={m} value={m}>{text.exports.modes[m]}</option>
          ))}
        </select>
      </label>
      <p className="muted">{t.purposeHint}</p>
      {dataset ? (
        <>
          <p className="muted">{all.privacy.dataset.deidentifiedOnly}</p>
          <label className="check">
            <input type="checkbox" name="include_labels" defaultChecked />
            {all.privacy.dataset.labels}
          </label>
          <p className="muted">{all.privacy.dataset.labelsHint}</p>
        </>
      ) : null}
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
      {destinations.length > 0 && dataset ? <p className="muted">{all.privacy.dataset.destinationsOff}</p> : null}
      {destinations.length > 0 && !dataset ? (
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
        <p role="alert" className="error">{t.failed} {failureText(state, text.exports.errors)}</p>
      ) : null}
    </form>
  );
}
