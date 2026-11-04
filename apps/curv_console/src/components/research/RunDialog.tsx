"use client";

import { useRouter } from "next/navigation";
import { useMemo, useState, useTransition } from "react";
import type { Cohort, CohortPreview, ParamRegistry, RUN_KINDS, SweepSpec } from "@/lib/api/research";
import { LEVEL_REQUESTS } from "@/lib/api/research";
import { formatValue, paramRegistry as bundledRegistry, stepOf, valueOf, type ProfileDoc } from "@/lib/research/registry";
import { failureText, fieldReasons, type ResearchFailure, type ResearchResult } from "@/lib/research/result";
import {
  axisKnobs,
  checkSweep,
  fieldTarget,
  MAX_SWEEP_AXES,
  MAX_SWEEP_CELLS,
  METRIC_KEYS,
  newAxis,
  sweepSpec,
  type AxisCheck,
  type AxisDraft,
} from "@/lib/research/run-sweep";
import { MAX_SWEEP_STEPS } from "@/lib/research/sweep";
import { researchText, type ResearchMessages } from "@/messages/research";
import r from "./research.module.css";

export type RunKind = (typeof RUN_KINDS)[number];

/** What the dialog asks the server action to start (`POST /analysis-runs`). */
export interface RunStartInput {
  kind: RunKind;
  experiment_id: string;
  cohort_id: string;
  level: string;
  seed?: number;
  sweep?: SweepSpec;
  options?: { metrics: string[] };
}

const KINDS: readonly RunKind[] = ["cohort_rescore", "param_sweep", "age_profile"];

/**
 * "Run on cohort" (RUI-5): pick a saved cohort, see its preview counts (suppressed bands flagged, PRV-1) and the budget
 * line, choose the kind (re-score, parameter sweep, age profile), the level and an optional seed, then start the run
 * (202) and go to its page. A sweep adds one or two axes bounded by the registry and the metrics to summarise per
 * cell (SWP-1). The run uses the saved experiment, so unsaved changes block it.
 */
export function RunDialog({
  orgId,
  experimentId,
  cohorts,
  dirty,
  previewCohort,
  runOnCohort,
  onClose,
  registry = bundledRegistry,
  publishedProfile = null,
  text = researchText,
}: {
  orgId: string;
  experimentId: string;
  cohorts: Cohort[];
  dirty: boolean;
  previewCohort: (definition: Cohort["definition"]) => Promise<ResearchResult<{ preview: CohortPreview }>>;
  runOnCohort: (input: RunStartInput) => Promise<ResearchResult<{ runId: string }>>;
  onClose: () => void;
  registry?: ParamRegistry;
  publishedProfile?: ProfileDoc | null;
  text?: ResearchMessages;
}) {
  const t = text.run;
  const s = text.runSetup;
  const router = useRouter();
  const [cohortId, setCohortId] = useState("");
  const [kind, setKind] = useState<RunKind>("cohort_rescore");
  const [level, setLevel] = useState<string>("auto");
  const [seed, setSeed] = useState("");
  const [axes, setAxes] = useState<AxisDraft[]>([]);
  const [metrics, setMetrics] = useState<string[]>([]);
  const [preview, setPreview] = useState<CohortPreview | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [serverFields, setServerFields] = useState<ResearchFailure["params"]>(undefined);
  const [pending, start] = useTransition();
  const cohort = cohorts.find((c) => c.id === cohortId);
  const published = useMemo<ProfileDoc>(() => publishedProfile ?? {}, [publishedProfile]);
  const knobs = useMemo(() => axisKnobs(registry.params, level), [registry, level]);
  const check = useMemo(() => checkSweep(registry.params, axes, level, published), [registry, axes, level, published]);
  const sweeping = kind === "param_sweep";
  const sweepReady = !sweeping || check.ok;

  const pickCohort = (id: string) => {
    setCohortId(id);
    setPreview(null);
    setError(null);
  };

  const edited = () => {
    setServerFields(undefined);
    setError(null);
  };

  const pickKind = (next: RunKind) => {
    setKind(next);
    edited();
    if (next === "param_sweep" && axes.length === 0 && knobs[0]) setAxes([newAxis(knobs[0])]);
  };

  /** A level change drops to a knob the new level allows: the frames-stage rows leave the list below level frames. */
  const pickLevel = (next: string) => {
    setLevel(next);
    edited();
    const allowed = axisKnobs(registry.params, next);
    setAxes((cur) => cur.map((a) => (allowed.some((k) => k.id === a.paramId) || !allowed[0] ? a : newAxis(allowed[0]))));
  };

  const setAxis = (i: number, patch: Partial<AxisDraft>) => {
    edited();
    setAxes((cur) => cur.map((a, j) => (j === i ? { ...a, ...patch } : a)));
  };

  const pickKnob = (i: number, id: string) => {
    const spec = registry.params.find((p) => p.id === id);
    if (spec) setAxis(i, newAxis(spec));
  };

  const addAxis = () => {
    const free = knobs.find((k) => !axes.some((a) => a.paramId === k.id));
    if (free) setAxes((cur) => [...cur, newAxis(free)]);
  };

  const toggleMetric = (key: string) => {
    edited();
    setMetrics((cur) => (cur.includes(key) ? cur.filter((k) => k !== key) : [...cur, key]));
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
    if (!cohort || dirty || !sweepReady) return;
    setError(null);
    setServerFields(undefined);
    const parsedSeed = seed.trim() === "" ? undefined : Number(seed);
    start(async () => {
      const res = await runOnCohort({
        kind,
        experiment_id: experimentId,
        cohort_id: cohort.id,
        level,
        ...(parsedSeed !== undefined && Number.isSafeInteger(parsedSeed) ? { seed: parsedSeed } : {}),
        ...(sweeping ? { sweep: sweepSpec(axes), options: { metrics } } : {}),
      });
      if (res.ok) router.push(`/${encodeURIComponent(orgId)}/research/runs/${encodeURIComponent(res.runId)}`);
      else {
        setServerFields(res.params);
        setError(failureText(res, text));
      }
    });
  }

  /** The text of an axis problem found on the client. */
  const problemText = (c: AxisCheck): string | null => {
    if (c.ok) return null;
    const p = c.problem;
    const e = s.problems;
    switch (p.reason) {
      case "bounds": return e.bounds(p.min, p.max);
      case "fine": return e.fine(p.step);
      case "too_many": return e.tooMany(p.steps, MAX_SWEEP_STEPS);
      default: return e[p.reason];
    }
  };

  /** The server's reasons for one target, shown where the field is. */
  const serverFor = (match: (target: ReturnType<typeof fieldTarget>) => boolean): string[] => {
    const rows = (serverFields ?? []).filter((p) => match(fieldTarget(p.name)));
    return Object.values(fieldReasons(rows));
  };

  const sweepProblem = check.problem === "axes" ? s.problems.axes : check.problem === "same_knob" ? s.problems.sameKnob : check.problem === "cells" ? s.problems.cells(check.cells ?? 0, MAX_SWEEP_CELLS) : null;

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
            {s.kind}
            <select value={kind} onChange={(ev) => pickKind(ev.currentTarget.value as RunKind)}>
              {KINDS.map((k) => <option key={k} value={k}>{s.kinds[k]}</option>)}
            </select>
          </label>
          <label>
            {t.level}
            <select value={level} onChange={(ev) => pickLevel(ev.currentTarget.value)}>
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
      {kind === "age_profile" ? <p className="muted" style={{ margin: 0 }}>{s.ageNote}</p> : null}
      {sweeping ? (
        <div className="stack" aria-label={s.axes}>
          <h3 style={{ margin: 0 }}>{s.axes}</h3>
          {level !== "frames" ? <p className="muted" style={{ margin: 0 }}>{s.framesOnly}</p> : null}
          {knobs.length === 0 ? <p className="muted">{s.noKnobs}</p> : null}
          {axes.map((axis, i) => {
            const spec = registry.params.find((p) => p.id === axis.paramId);
            const c = check.axes[i];
            const reasons = [...(c ? [problemText(c)] : []), ...serverFor((x) => x.kind === "axis" && x.index === i)].filter((x): x is string => Boolean(x));
            const pub = spec ? valueOf(published, spec) : undefined;
            const kindSpec = spec?.kind;
            const bounds = kindSpec && (kindSpec.type === "number" || kindSpec.type === "integer") ? { min: kindSpec.min, max: kindSpec.max } : {};
            return (
              <fieldset key={i} className={r.row} aria-label={s.axis(i + 1)} aria-invalid={reasons.length > 0}>
                <legend>{s.axis(i + 1)}</legend>
                <label>
                  {s.knob}
                  <select value={axis.paramId} onChange={(ev) => pickKnob(i, ev.currentTarget.value)}>
                    {knobs.map((k) => <option key={k.id} value={k.id}>{text.params[k.id]?.label ?? k.id}</option>)}
                  </select>
                </label>
                {(["from", "to", "step"] as const).map((f) => (
                  <label key={f}>
                    {s[f]}
                    <input
                      type="number"
                      className={r.ageInput}
                      step={spec ? stepOf(spec.kind) : "any"}
                      {...(f === "step" ? {} : bounds)}
                      value={axis[f]}
                      onChange={(ev) => setAxis(i, { [f]: ev.currentTarget.value })}
                    />
                  </label>
                ))}
                {axes.length > 1 ? <button type="button" className="secondary" onClick={() => { edited(); setAxes((cur) => cur.filter((_, j) => j !== i)); }}>{s.removeAxis}</button> : null}
                {pub !== undefined && typeof pub === "number" ? <p className="muted" style={{ margin: 0, flexBasis: "100%" }}>{s.published(formatValue(pub))}</p> : null}
                {reasons.map((m) => <p key={m} role="alert" className={r.error} style={{ margin: 0, flexBasis: "100%" }}>{m}</p>)}
              </fieldset>
            );
          })}
          {axes.length < MAX_SWEEP_AXES && knobs.length > axes.length ? (
            <div className={r.row}><button type="button" className="secondary" onClick={addAxis}>{s.addAxis}</button></div>
          ) : null}
          <p role="status" style={{ margin: 0 }}>{check.cells === null ? s.cellsUnknown : s.cells(check.cells, MAX_SWEEP_CELLS)}</p>
          {[sweepProblem, ...serverFor((x) => x.kind === "sweep")].filter((x): x is string => Boolean(x)).map((m) => <p key={m} role="alert" className={r.error} style={{ margin: 0 }}>{m}</p>)}
          <fieldset aria-label={s.metrics}>
            <legend>{s.metrics}</legend>
            <p className="muted" style={{ margin: 0 }}>{s.metricsHint}</p>
            <div className={r.row}>
              {METRIC_KEYS.map((k) => (
                <label key={k}>
                  <input type="checkbox" checked={metrics.includes(k)} onChange={() => toggleMetric(k)} /> {k}
                </label>
              ))}
            </div>
            {serverFor((x) => x.kind === "metrics").map((m) => <p key={m} role="alert" className={r.error} style={{ margin: 0 }}>{m}</p>)}
          </fieldset>
        </div>
      ) : null}
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
            {t.budget(preview.n == null ? text.suppressedCell(preview.small_cell_k) : String(preview.n), sweeping ? (check.cells ?? 1) : 1)}
          </p>
        </div>
      ) : null}
      {error ? <p role="alert" className="muted">{error}</p> : null}
      <div className={r.row}>
        <button type="button" disabled={!cohort || dirty || pending || !sweepReady} onClick={submit}>{pending ? t.submitting : t.submit}</button>
        <button type="button" className="secondary" onClick={onClose}>{t.cancel}</button>
      </div>
    </section>
  );
}
