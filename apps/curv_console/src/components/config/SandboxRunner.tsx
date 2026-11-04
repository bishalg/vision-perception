"use client";

import { useState, useTransition } from "react";
import type { SandboxSession } from "@/app/[org]/config/docActions";
import s from "@/components/catalog/catalog.module.css";
import { catalogFailureText, type CatalogResult } from "@/components/catalog/types";
import { FlagBadge } from "@/components/FlagBadge";
import type { SessionSummary } from "@/lib/api";
import type { ConfigDocumentKind } from "@/lib/api/catalog";
import { browserCore } from "@/lib/asd/browser";
import { defaultScoringProfile, rescoreSession, type ContractInfo, type RescoreResult } from "@/lib/asd/ops";
import { recordSandboxRun, summarizeSandbox, type SandboxOutcome } from "@/lib/configDocs";
import { formatNumber } from "@/lib/format";
import c from "./config.module.css";
import { useContractInfo } from "./DraftShell";
import { useText } from "@/lib/i18n/provider";


export interface SandboxSide {
  label: string;
  profile: unknown;
  baselines: unknown;
}

interface Row {
  session: SessionSummary;
  age: number;
  before: RescoreResult | null;
  after: RescoreResult | null;
  error: string | null;
}

function Caveats({ info }: { info: ContractInfo | null }) {
  const tx = useText();
  const t = tx.config.sandbox;
  const fixed = info ? String(info.lowConfidenceValidRatio) : "0.6";
  return (
    <section className="panel" aria-label={t.caveats.title}>
      <h3 style={{ marginTop: 0 }}>{t.caveats.title}</h3>
      <ul className={c.caveats}>
        <li>{t.caveats.lowConfidence(fixed)}</li>
        <li>{t.caveats.unreliable}</li>
        <li>{t.caveats.perClip}</li>
        <li>{t.caveats.device}</li>
      </ul>
    </section>
  );
}

const composite = (r: RescoreResult | null) => r?.score.compositeScore ?? null;

/**
 * Sandbox re-score (spec 03 §4.5, IA `/config/sandbox`): re-scores the chosen sessions with the `published` and the
 * `draft` documents through the Rust core in the browser (asd-wasm `rescoreSession` over the stored record) and shows the per-session
 * flag and composite change and a summary. Writes nothing; `record` remembers the run in this browser for the
 * publish gate of those versions.
 */
export function SandboxRunner({
  sessions,
  initialSelected,
  published,
  draft,
  load,
  record,
  defaultAge = 30,
}: {
  sessions: SessionSummary[];
  initialSelected: string[];
  published: SandboxSide | null;
  draft: SandboxSide;
  load: (ids: string[]) => Promise<CatalogResult<{ sessions: SandboxSession[] }>>;
  record?: { kind: ConfigDocumentKind; version: string }[];
  defaultAge?: number;
}) {
  const tx = useText();
  const t = tx.config.sandbox;
  const [selected, setSelected] = useState<Set<string>>(() => new Set(initialSelected));
  const [age, setAge] = useState(defaultAge);
  const [ages, setAges] = useState<Record<string, number>>({});
  const [rows, setRows] = useState<Row[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const core = useContractInfo();
  const info = core.info;
  const [lost, setUnavailable] = useState(false);
  const unavailable = core.unavailable || lost;
  const [pending, start] = useTransition();

  const ageOf = (id: string) => ages[id] ?? age;
  const toggle = (id: string) =>
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });

  function run() {
    setError(null);
    if (!published) {
      setError(t.noBaselines);
      return;
    }
    const ids = sessions.map((x) => x.id).filter((id) => selected.has(id));
    if (!ids.length) {
      setError(t.pickSessions);
      return;
    }
    start(async () => {
      const loaded = await load(ids);
      if (!loaded.ok) {
        setError(`${t.loadFailed} ${catalogFailureText(loaded, tx.catalog.errors)}`);
        return;
      }
      const core = browserCore();
      // A side without a scoring profile (none published yet) uses the Rust core's default.
      let fallback: unknown = null;
      if (published.profile === null || draft.profile === null) {
        const d = await defaultScoringProfile(core);
        fallback = d.ok ? d.result : null;
      }
      const left = { profile: published.profile ?? fallback, baselines: published.baselines };
      const right = { profile: draft.profile ?? fallback, baselines: draft.baselines };
      const out: Row[] = [];
      let panicked = false;
      for (const { id, record } of loaded.sessions) {
        const summary = sessions.find((x) => x.id === id);
        if (!summary) continue;
        const opts = { ageMonths: ageOf(id) };
        const [before, after] = [
          await rescoreSession(core, record, left.profile, left.baselines, opts),
          await rescoreSession(core, record, right.profile, right.baselines, opts),
        ];
        for (const r of [before, after]) {
          if (!r.ok && r.error.kind === "panic") panicked = true;
          if (!r.ok && r.error.kind === "unavailable") setUnavailable(true);
        }
        out.push({
          session: summary,
          age: opts.ageMonths,
          before: before.ok ? before.result : null,
          after: after.ok ? after.result : null,
          error: !before.ok ? before.error.message : !after.ok ? after.error.message : null,
        });
      }
      setRows(out);
      if (panicked) setError(t.corePanic);
      const summary = summarizeSandbox(outcomes(out));
      if (record && summary.sessions > summary.failed) {
        const at = new Date().toISOString();
        for (const r of record) recordSandboxRun(r.kind, r.version, { at, sessions: summary.sessions, changed: summary.changed });
      }
    });
  }

  const summary = rows ? summarizeSandbox(outcomes(rows)) : null;

  return (
    <div className="stack">
      <p className="muted" style={{ margin: 0 }}>
        {unavailable ? <span className={s.alert} role="alert">{t.coreUnavailable}</span> : info ? t.core(info.coreVersion) : null}
      </p>
      <Caveats info={info} />

      <section className="panel stack" aria-label={t.sessions}>
        <h3 style={{ margin: 0 }}>{t.sessions}</h3>
        {sessions.length === 0 ? <p className="muted">{t.noSessions}</p> : null}
        <div className={s.row}>
          <label>
            {t.defaultAge}
            <input type="number" min={0} max={240} className={c.ageInput} value={age}
              onChange={(ev) => setAge(Math.max(0, Math.round(ev.currentTarget.valueAsNumber || 0)))} />
          </label>
          <button type="button" className={s.small} onClick={() => setSelected(new Set(sessions.map((x) => x.id)))}>{t.selectAll}</button>
          <button type="button" className={s.small} onClick={() => setSelected(new Set())}>{t.selectNone}</button>
          <span className="muted">{t.selected(selected.size)}</span>
        </div>
        <p className="muted" style={{ margin: 0 }}>{t.ageHint}</p>
        <div className={`table-wrap ${c.sessionList}`}>
          <table className="data">
            <thead>
              <tr><th /><th>{t.cols.session}</th><th>{t.cols.participant}</th><th>{t.cols.recorded}</th><th>{t.cols.age}</th></tr>
            </thead>
            <tbody>
              {sessions.map((x) => (
                <tr key={x.id}>
                  <td>
                    <input type="checkbox" aria-label={`${t.cols.session} ${x.id}`} checked={selected.has(x.id)} onChange={() => toggle(x.id)} />
                  </td>
                  <td>{x.id}</td>
                  <td>{x.participant_token}</td>
                  <td><FlagBadge flag={x.attention_flag} /> {formatNumber(x.composite_score, 2)}</td>
                  <td>
                    <input type="number" min={0} max={240} className={c.ageInput} aria-label={`${t.age} ${x.id}`} value={ageOf(x.id)}
                      onChange={(ev) => {
                        const v = Math.max(0, Math.round(ev.currentTarget.valueAsNumber || 0));
                        setAges((a) => ({ ...a, [x.id]: v }));
                      }} />
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
        <div className={s.row}>
          <button type="button" disabled={pending || unavailable} onClick={run}>{pending ? t.running : t.run}</button>
        </div>
        {error ? <p role="alert" className={s.alert}>{error}</p> : null}
      </section>

      {summary ? (
        <section className="panel stack" aria-label={t.summary.title}>
          <h3 style={{ margin: 0 }}>{t.summary.title}</h3>
          <div className={c.summary}>
            <div><div className="muted">{t.summary.sessions}</div><div className={c.stat}>{summary.sessions - summary.failed}</div></div>
            <div><div className="muted">{t.summary.changed}</div><div className={c.stat} data-testid="flags-changed">{summary.changed}</div></div>
            <div><div className="muted">{t.summary.failed}</div><div className={c.stat}>{summary.failed}</div></div>
            <div><div className="muted">{t.summary.meanDelta}</div><div className={c.stat}>{signed(summary.meanDelta)}</div></div>
          </div>
          <div>
            <strong>{t.summary.transitions}:</strong>{" "}
            {Object.keys(summary.transitions).length
              ? Object.entries(summary.transitions).map(([k, n]) => `${k} ×${n}`).join(", ")
              : t.summary.none}
          </div>
        </section>
      ) : null}

      {rows ? (
        <div className="table-wrap">
          <table className="data" aria-label={t.title}>
            <thead>
              <tr>
                <th>{t.cols.session}</th>
                <th>{t.cols.age}</th>
                <th>{t.cols.recorded}</th>
                <th>{t.cols.published} ({published?.label})</th>
                <th>{t.cols.draft} ({draft.label})</th>
                <th>{t.cols.delta}</th>
                <th>{t.cols.change}</th>
                <th>{t.cols.notes}</th>
              </tr>
            </thead>
            <tbody>
              {rows.map((r) => {
                const changed = r.before && r.after && r.before.flag.flag !== r.after.flag.flag;
                const b = composite(r.before);
                const a = composite(r.after);
                const notes = [...new Set([...(r.before?.warnings ?? []), ...(r.after?.warnings ?? [])])];
                return (
                  <tr key={r.session.id} className={changed ? c.changed : undefined}>
                      <td>{r.session.id}</td>
                      <td>{r.age}</td>
                      <td><FlagBadge flag={r.session.attention_flag} /> {formatNumber(r.session.composite_score, 2)}</td>
                      <td>{r.before ? <><FlagBadge flag={r.before.flag.flag} /> {formatNumber(b, 3)}</> : t.error}</td>
                      <td>{r.after ? <><FlagBadge flag={r.after.flag.flag} /> {formatNumber(a, 3)}</> : t.error}</td>
                      <td>{b !== null && a !== null ? signed(a - b) : "—"}</td>
                      <td>{r.error ? t.error : changed ? `${r.before!.flag.flag} → ${r.after!.flag.flag}` : t.unchanged}</td>
                      <td style={{ whiteSpace: "normal" }}>
                        {r.error ? <span className={s.fieldError}>{r.error}</span> : null}
                        {notes.length ? (
                          <details>
                            <summary>{t.warnings(notes.length)}</summary>
                            <ul className={c.problems}>{notes.map((w) => <li key={w}>{w}</li>)}</ul>
                          </details>
                        ) : null}
                      </td>
                    </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      ) : null}
    </div>
  );
}

function outcomes(rows: Row[]): SandboxOutcome[] {
  return rows.map((r) => ({
    sessionId: r.session.id,
    before: r.before ? { flag: r.before.flag.flag, composite: composite(r.before) } : null,
    after: r.after ? { flag: r.after.flag.flag, composite: composite(r.after) } : null,
  }));
}

function signed(v: number | null): string {
  if (v === null) return "—";
  const r = Math.round(v * 1000) / 1000;
  return `${r > 0 ? "+" : ""}${r.toFixed(3)}`;
}
