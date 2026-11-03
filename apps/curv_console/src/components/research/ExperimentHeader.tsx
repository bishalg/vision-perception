"use client";

import { useState } from "react";
import type { Experiment } from "@/lib/api/research";
import { researchText, type ResearchMessages } from "@/messages/research";
import r from "./research.module.css";

/**
 * Name, description, provenance line and the actions of the workbench (save, run on cohort, promote). Presentational:
 * the workbench owns the state and the server calls.
 */
export function ExperimentHeader({
  experiment,
  name,
  description,
  onName,
  onDescription,
  readOnly,
  saving,
  canSave,
  onSave,
  canRun,
  onRun,
  canPromote,
  onPromote,
  promoteError,
  text = researchText,
}: {
  experiment: Experiment;
  name: string;
  description: string;
  onName: (v: string) => void;
  onDescription: (v: string) => void;
  readOnly: boolean;
  saving: boolean;
  canSave: boolean;
  onSave: () => void;
  canRun: boolean;
  onRun: () => void;
  canPromote: boolean;
  onPromote: (input: { version: string; notes: string }) => void;
  promoteError: string | null;
  text?: ResearchMessages;
}) {
  const w = text.workbench;
  const [promoting, setPromoting] = useState(false);
  const [version, setVersion] = useState("");
  const [notes, setNotes] = useState("");
  return (
    <section className="panel stack" aria-label={w.title}>
      <div className={r.row}>
        <label>
          {w.nameLabel}
          <input value={name} maxLength={200} disabled={readOnly} onChange={(ev) => onName(ev.currentTarget.value)} />
        </label>
        <label style={{ flex: 1 }}>
          {w.descriptionLabel}
          <input value={description} maxLength={2000} disabled={readOnly} onChange={(ev) => onDescription(ev.currentTarget.value)} />
        </label>
      </div>
      <p className="muted" style={{ margin: 0 }}>
        {w.base(experiment.base_profile_version)} · {text.experiments.status[experiment.status]} · {w.profileHash}: <code>{experiment.profile_hash.slice(0, 12)}</code>
      </p>
      {readOnly ? <p className="muted">{w.readOnly}</p> : null}
      <div className={r.row}>
        <button type="button" disabled={!canSave || saving} onClick={onSave}>{saving ? w.saving : w.save}</button>
        <button type="button" className="secondary" disabled={!canRun} onClick={onRun}>{w.runOnCohort}</button>
        <button type="button" className="secondary" disabled={!canPromote} onClick={() => setPromoting((v) => !v)}>{w.promote}</button>
      </div>
      {!experiment.last_run_id && canPromote ? <p className="muted" style={{ margin: 0 }}>{w.promoteNoRun}</p> : null}
      {promoting ? (
        <form
          className="stack"
          onSubmit={(ev) => {
            ev.preventDefault();
            if (version.trim()) onPromote({ version: version.trim(), notes: notes.trim() });
          }}
        >
          <label>
            {w.promoteVersion}
            <input value={version} onChange={(ev) => setVersion(ev.currentTarget.value)} required />
          </label>
          <label>
            {w.promoteNotes}
            <textarea rows={2} value={notes} maxLength={2000} onChange={(ev) => setNotes(ev.currentTarget.value)} />
          </label>
          <div className={r.row}>
            <button type="submit">{w.promoteSubmit}</button>
            <button type="button" className="secondary" onClick={() => setPromoting(false)}>{w.promoteCancel}</button>
          </div>
          {promoteError ? <p role="alert" className="muted">{promoteError}</p> : null}
        </form>
      ) : null}
    </section>
  );
}
