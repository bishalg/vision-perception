"use client";

import { useRouter } from "next/navigation";
import { useState, useTransition } from "react";
import type { Cohort, CohortPreview } from "@/lib/api/research";
import { LEVEL_REQUESTS } from "@/lib/api/research";
import { failureText, type ResearchResult } from "@/lib/research/result";
import { researchText, type ResearchMessages } from "@/messages/research";
import r from "./research.module.css";

/**
 * "Run on cohort" (RUI-5): pick a saved cohort, see its preview counts (suppressed bands flagged, PRV-1) and the budget
 * line, choose the level and an optional seed, then start the run (202) and go to its page. The run uses the saved
 * experiment, so unsaved changes block it.
 */
export function RunDialog({
  orgId,
  experimentId,
  cohorts,
  dirty,
  previewCohort,
  runOnCohort,
  onClose,
  text = researchText,
}: {
  orgId: string;
  experimentId: string;
  cohorts: Cohort[];
  dirty: boolean;
  previewCohort: (definition: Cohort["definition"]) => Promise<ResearchResult<{ preview: CohortPreview }>>;
  runOnCohort: (input: { experiment_id: string; cohort_id: string; level: string; seed?: number }) => Promise<ResearchResult<{ runId: string }>>;
  onClose: () => void;
  text?: ResearchMessages;
}) {
  const t = text.run;
  const router = useRouter();
  const [cohortId, setCohortId] = useState("");
  const [level, setLevel] = useState<string>("auto");
  const [seed, setSeed] = useState("");
  const [preview, setPreview] = useState<CohortPreview | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [pending, start] = useTransition();
  const cohort = cohorts.find((c) => c.id === cohortId);

  const pickCohort = (id: string) => {
    setCohortId(id);
    setPreview(null);
    setError(null);
  };

  function countPreview() {
    if (!cohort) return;
    setError(null);
    start(async () => {
      const res = await previewCohort(cohort.definition);
      if (res.ok) setPreview(res.preview);
      else setError(failureText(res, text));
    });
  }

  function submit() {
    if (!cohort || dirty) return;
    setError(null);
    const parsedSeed = seed.trim() === "" ? undefined : Number(seed);
    start(async () => {
      const res = await runOnCohort({
        experiment_id: experimentId,
        cohort_id: cohort.id,
        level,
        ...(parsedSeed !== undefined && Number.isInteger(parsedSeed) ? { seed: parsedSeed } : {}),
      });
      if (res.ok) router.push(`/${encodeURIComponent(orgId)}/research/runs/${encodeURIComponent(res.runId)}`);
      else setError(failureText(res, text));
    });
  }

  return (
    <section role="dialog" aria-modal="false" aria-labelledby="run-dialog-title" className={`panel stack ${r.dialog}`}>
      <h2 id="run-dialog-title" style={{ margin: 0 }}>{t.title}</h2>
      <p className="muted" style={{ margin: 0 }}>{t.intro}</p>
      {dirty ? <p className="muted">{t.saveFirst}</p> : null}
      {cohorts.length === 0 ? (
        <p className="muted">{t.noCohorts}</p>
      ) : (
        <div className={r.row}>
          <label>
            {t.cohort}
            <select value={cohortId} onChange={(ev) => pickCohort(ev.currentTarget.value)}>
              <option value="">{t.cohortPick}</option>
              {cohorts.map((c) => <option key={c.id} value={c.id}>{c.name}</option>)}
            </select>
          </label>
          <label>
            {t.level}
            <select value={level} onChange={(ev) => setLevel(ev.currentTarget.value)}>
              {LEVEL_REQUESTS.map((l) => <option key={l} value={l}>{text.level[l]}</option>)}
            </select>
          </label>
          <label>
            {t.seed}
            <input inputMode="numeric" value={seed} onChange={(ev) => setSeed(ev.currentTarget.value)} />
          </label>
        </div>
      )}
      <p className="muted" style={{ margin: 0 }}>{t.seedHint}</p>
      <div className={r.row}>
        <button type="button" className="secondary" disabled={!cohort || pending} onClick={countPreview}>{pending ? t.previewing : t.preview}</button>
      </div>
      {preview ? (
        <div role="status">
          <p style={{ margin: 0 }}>
            {t.previewCounts(preview.n == null ? text.suppressedCell(preview.small_cell_k) : String(preview.n), preview.by_level.frames, preview.by_level.score)}
          </p>
          <ul>
            {preview.by_age_band.map((b) => (
              <li key={b.band}>{t.bandRow(b.band, b.n == null ? text.suppressedCell(preview.small_cell_k) : String(b.n))}</li>
            ))}
          </ul>
          <p className="muted" style={{ margin: 0 }}>
            {t.excluded(preview.excluded.no_consent, preview.excluded.withdrawn, preview.excluded.unscorable, preview.excluded.no_age, preview.excluded.quality)}
          </p>
          <p className="muted" style={{ margin: 0 }}>
            {t.budget(preview.n == null ? text.suppressedCell(preview.small_cell_k) : String(preview.n), 1)}
          </p>
        </div>
      ) : null}
      {error ? <p role="alert" className="muted">{error}</p> : null}
      <div className={r.row}>
        <button type="button" disabled={!cohort || dirty || pending} onClick={submit}>{pending ? t.submitting : t.submit}</button>
        <button type="button" className="secondary" onClick={onClose}>{t.cancel}</button>
      </div>
    </section>
  );
}
