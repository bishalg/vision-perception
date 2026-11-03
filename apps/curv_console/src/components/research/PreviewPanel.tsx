"use client";

import { useEffect, useMemo, useRef, useState, useTransition } from "react";
import type { PreviewRecord } from "@/app/[org]/research/actions";
import type { ParamRegistry } from "@/lib/api/research";
import { browserCore } from "@/lib/asd/browser";
import type { AsdCore } from "@/lib/asd/core";
import { defaultScoringProfile, reprocessSession, rescoreSession } from "@/lib/asd/ops";
import {
  MAX_PREVIEW_SESSIONS,
  debounce,
  parseTrace,
  paramsNotApplied,
  summarizeRescore,
  type ScoreSide,
} from "@/lib/research/preview";
import { formatValue, isNumeric, setAt, stepOf, sweepableParams, valueOf, type ProfileDoc } from "@/lib/research/registry";
import type { ResearchResult } from "@/lib/research/result";
import { MAX_SWEEP_STEPS, defaultSweep, sweepValues } from "@/lib/research/sweep";
import { researchText, type ResearchMessages } from "@/messages/research";
import { PreviewCard, type CardData } from "./PreviewCard";
import { SessionPicker, type PickerSession } from "./SessionPicker";
import { SweepCurve, type CurvePoint } from "./SweepCurve";
import r from "./research.module.css";

export interface PreviewPanelProps {
  registry: ParamRegistry;
  /** The published scoring profile body; `null` when none is published (the core's default is used). */
  publishedProfile: ProfileDoc | null;
  /** The published normative baselines body; `null` disables the preview. */
  baselines: unknown | null;
  catalog?: unknown;
  draft: ProfileDoc;
  sessions: PickerSession[];
  loadRecords: (ids: string[]) => Promise<ResearchResult<{ records: PreviewRecord[] }>>;
  loadFrames: (id: string) => Promise<ResearchResult<{ jsonl: string }>>;
  core?: AsdCore;
  text?: ResearchMessages;
}

const DEFAULT_AGE_MONTHS = 30;

type Side = { score: ScoreSide; trace: ReturnType<typeof parseTrace> };

/**
 * Live preview (RUI-3) and single-session sweep (RUI-4): re-scores the picked sessions in the browser through the Rust
 * core with the published profile and with the unsaved draft, 150 ms after the last change. The stored records are read
 * once per session (server action) and kept in memory only. Frame-level re-processing needs the core's
 * `reprocessSession` (WP-X.4); until then the score-level result of `rescoreSession` is shown.
 */
export function PreviewPanel(props: PreviewPanelProps) {
  const { registry, publishedProfile, baselines, draft, sessions, loadRecords, loadFrames, catalog } = props;
  const text = props.text ?? researchText;
  const p = text.preview;
  const core = useMemo(() => props.core ?? browserCore(), [props.core]);
  const [selected, setSelected] = useState<Set<string>>(() => new Set(sessions.slice(0, 2).map((s) => s.id)));
  const [ages, setAges] = useState<Record<string, number>>({});
  const [defaultAge, setDefaultAge] = useState(DEFAULT_AGE_MONTHS);
  const [cards, setCards] = useState<CardData[]>([]);
  const [note, setNote] = useState<string | null>(null);
  const [, startTransition] = useTransition();

  const records = useRef(new Map<string, Record<string, unknown>>());
  const publishedCache = useRef(new Map<string, Side>());
  const frameState = useRef(new Map<string, string | null>());
  const fallback = useRef<ProfileDoc | null>(null);

  const ageOf = (id: string) => ages[id] ?? defaultAge;
  const ids = sessions.filter((s) => selected.has(s.id)).map((s) => s.id);
  const draftKey = JSON.stringify(draft);
  const runKey = `${draftKey}|${ids.join(",")}|${ids.map(ageOf).join(",")}|${baselines ? 1 : 0}`;

  async function publishedDoc(): Promise<ProfileDoc | null> {
    if (publishedProfile) return publishedProfile;
    if (!fallback.current) {
      const d = await defaultScoringProfile(core);
      if (d.ok) fallback.current = d.result as ProfileDoc;
    }
    return fallback.current;
  }

  /** Records of the picked sessions, read once each. */
  async function ensureRecords(want: string[]): Promise<string | null> {
    const missing = want.filter((id) => !records.current.has(id));
    if (!missing.length) return null;
    const loaded = await loadRecords(missing);
    if (!loaded.ok) return p.loadFailed;
    for (const rec of loaded.records) records.current.set(rec.id, rec.record);
    return null;
  }

  // The latest `run` is kept in a ref so the debounced timer always calls the current closure.
  const latest = useRef<() => Promise<void>>(async () => undefined);
  const token = useRef(0);
  const compute = async () => {
    const mine = ++token.current;
    if (!baselines || !ids.length) {
      setCards([]);
      return;
    }
    const failed = await ensureRecords(ids);
    if (failed) {
      setNote(failed);
      return;
    }
    const pub = await publishedDoc();
    if (!pub) {
      setNote(p.coreUnavailable);
      return;
    }
    let message: string | null = null;
    const next: CardData[] = [];
    for (const s of sessions.filter((x) => selected.has(x.id))) {
      const record = records.current.get(s.id);
      const opts = { ageMonths: ageOf(s.id) };
      let published = publishedCache.current.get(`${s.id}|${opts.ageMonths}`) ?? null;
      let error: string | null = null;
      if (!published && record) {
        const res = await rescoreSession(core, record, pub, baselines, opts);
        if (res.ok) {
          published = { score: summarizeRescore(res.result), trace: null };
          publishedCache.current.set(`${s.id}|${opts.ageMonths}`, published);
        } else {
          error = res.error.message;
          message = res.error.kind === "unavailable" ? p.coreUnavailable : res.error.kind === "panic" ? p.corePanic : message;
        }
      }
      let draftSide: Side | null = null;
      if (record) {
        const res = await rescoreSession(core, record, draft, baselines, opts);
        if (res.ok) draftSide = { score: summarizeRescore(res.result), trace: null };
        else {
          error ??= res.error.message;
          message = res.error.kind === "unavailable" ? p.coreUnavailable : res.error.kind === "panic" ? p.corePanic : message;
        }
      }
      // Frame level: the research log is read once per session (audited), then re-processed when the core can.
      let framesNote = false;
      let publishedTrace = null as Side["trace"];
      let draftTrace = null as Side["trace"];
      if (s.framesAvailable) {
        if (!frameState.current.has(s.id)) {
          const f = await loadFrames(s.id);
          frameState.current.set(s.id, f.ok ? f.jsonl : null);
        }
        const jsonl = frameState.current.get(s.id) ?? null;
        if (jsonl !== null) {
          const [a, b] = [
            await reprocessSession(core, jsonl, catalog ?? null, pub, baselines, opts),
            await reprocessSession(core, jsonl, catalog ?? null, draft, baselines, opts),
          ];
          if (a.ok && b.ok) {
            publishedTrace = parseTrace(a.result.trace);
            draftTrace = parseTrace(b.result.trace);
          }
        }
        framesNote = !publishedTrace && !draftTrace;
      }
      const level = s.framesAvailable && !framesNote ? "frames" : "score";
      next.push({
        id: s.id,
        token: s.token,
        framesAvailable: s.framesAvailable,
        published: published?.score ?? null,
        draft: draftSide?.score ?? null,
        notApplied: paramsNotApplied(registry.params, pub, draft, level),
        framesNote,
        publishedTrace,
        draftTrace,
        aois: [],
        error,
      });
    }
    if (mine !== token.current) return;
    startTransition(() => {
      setCards(next);
      setNote(message);
    });
  };

  useEffect(() => {
    latest.current = compute;
  });

  useEffect(() => {
    const run = debounce(() => void latest.current());
    run();
    return () => run.cancel();
  }, [runKey]);

  const toggle = (id: string) =>
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else if (next.size < MAX_PREVIEW_SESSIONS) next.add(id);
      return next;
    });

  return (
    <section className="panel stack" aria-label={p.title}>
      <h2 style={{ margin: 0 }}>{p.title}</h2>
      <p className="muted" style={{ margin: 0 }}>{p.intro}</p>
      {!baselines ? <p className="muted">{p.noBaselines}</p> : null}
      <SessionPicker
        sessions={sessions}
        selected={selected}
        onToggle={toggle}
        ages={ages}
        defaultAge={defaultAge}
        onAge={(id, age) => setAges((a) => ({ ...a, [id]: age }))}
        onDefaultAge={setDefaultAge}
        max={MAX_PREVIEW_SESSIONS}
        text={text}
      />
      {note ? <p role="alert" className="muted">{note}</p> : null}
      <div className={r.cards}>
        {cards.map((c) => <PreviewCard key={c.id} card={c} text={text} />)}
      </div>
      <SweepSection
        registry={registry}
        publishedProfile={publishedProfile}
        draft={draft}
        baselines={baselines}
        sessions={sessions.filter((s) => selected.has(s.id))}
        ageOf={ageOf}
        getRecord={async (id) => ((await ensureRecords([id])) ? null : (records.current.get(id) ?? null))}
        core={core}
        text={text}
      />
    </section>
  );
}

function SweepSection({
  registry,
  publishedProfile,
  draft,
  baselines,
  sessions,
  ageOf,
  getRecord,
  core,
  text,
}: {
  registry: ParamRegistry;
  publishedProfile: ProfileDoc | null;
  draft: ProfileDoc;
  baselines: unknown | null;
  sessions: PickerSession[];
  ageOf: (id: string) => number;
  getRecord: (id: string) => Promise<Record<string, unknown> | null>;
  core: AsdCore;
  text: ResearchMessages;
}) {
  const w = text.sweep;
  const knobs = useMemo(() => sweepableParams(registry.params), [registry]);
  const [knobId, setKnobId] = useState(knobs[0]?.id ?? "");
  const knob = knobs.find((k) => k.id === knobId);
  const [range, setRange] = useState(() => (knob ? defaultSweep(knob.kind) : null));
  const [sessionId, setSessionId] = useState("");
  const [result, setResult] = useState<{ points: CurvePoint[]; rows: { value: number; composite: number | null; flag: string }[]; label: string; published: number } | null>(null);
  const [problem, setProblem] = useState<string | null>(null);
  const [pending, start] = useTransition();

  if (!knob || !range) return <section aria-label={w.title}><h3>{w.title}</h3><p className="muted">{w.noKnobs}</p></section>;
  const sid = sessions.find((s) => s.id === sessionId)?.id ?? sessions[0]?.id ?? "";
  const label = text.params[knob.id]?.label ?? knob.id;
  const num = (v: string) => Number(v);

  function run() {
    if (!knob || !range) return;
    setProblem(null);
    const publishedValue = valueOf(publishedProfile ?? {}, knob);
    const values = sweepValues(knob.kind, range.from, range.to, range.step, typeof publishedValue === "number" ? publishedValue : Number.NaN);
    if (!values.ok) {
      setProblem(values.reason === "too_many" ? w.tooMany(MAX_SWEEP_STEPS) : w.invalid);
      return;
    }
    start(async () => {
      const record = await getRecord(sid);
      if (!record || !baselines) {
        setProblem(text.preview.loadFailed);
        return;
      }
      const rows: { value: number; composite: number | null; flag: string }[] = [];
      for (const v of values.values) {
        const res = await rescoreSession(core, record, setAt(draft, knob.id, v), baselines, { ageMonths: ageOf(sid) });
        if (!res.ok) {
          setProblem(text.preview.failed(res.error.message));
          return;
        }
        rows.push({ value: v, composite: res.result.score.compositeScore, flag: text.preview.flagLabel[res.result.flag.flag] });
      }
      setResult({
        points: rows.map((x) => ({ x: x.value, y: x.composite })),
        rows,
        label,
        published: typeof publishedValue === "number" ? publishedValue : Number.NaN,
      });
    });
  }

  return (
    <section className="stack" aria-label={w.title}>
      <h3 style={{ margin: 0 }}>{w.title}</h3>
      <p className="muted" style={{ margin: 0 }}>{w.intro}</p>
      {sessions.length === 0 ? <p className="muted">{w.noSession}</p> : null}
      <div className={r.row}>
        <label>
          {w.knob}
          <select value={knobId} onChange={(ev) => { const k = knobs.find((x) => x.id === ev.currentTarget.value); setKnobId(ev.currentTarget.value); setRange(k ? defaultSweep(k.kind) : null); setResult(null); }}>
            {knobs.map((k) => <option key={k.id} value={k.id}>{text.params[k.id]?.label ?? k.id}</option>)}
          </select>
        </label>
        <label>
          {w.session}
          <select value={sid} onChange={(ev) => setSessionId(ev.currentTarget.value)}>
            {sessions.map((s) => <option key={s.id} value={s.id}>{s.token}</option>)}
          </select>
        </label>
        {(["from", "to", "step"] as const).map((k) => (
          <label key={k}>
            {w[k]}
            <input
              type="number"
              className={r.ageInput}
              step={isNumeric(knob.kind) ? stepOf(knob.kind) : "any"}
              value={range[k]}
              onChange={(ev) => setRange({ ...range, [k]: num(ev.currentTarget.value) })}
            />
          </label>
        ))}
        <button type="button" disabled={pending || !sid || !baselines} onClick={run}>{pending ? w.running : w.run}</button>
      </div>
      {problem ? <p role="alert" className="muted">{problem}</p> : null}
      {result ? (
        <>
          <SweepCurve points={result.points} published={result.published} label={result.label} text={text} />
          <table className="data" aria-label={w.curve(result.label)}>
            <thead><tr><th>{w.cols.value}</th><th>{w.cols.composite}</th><th>{w.cols.flag}</th></tr></thead>
            <tbody>
              {result.rows.map((x) => (
                <tr key={x.value}><td>{formatValue(x.value)}</td><td>{x.composite === null ? "-" : formatValue(x.composite)}</td><td>{x.flag}</td></tr>
              ))}
            </tbody>
          </table>
        </>
      ) : null}
    </section>
  );
}
