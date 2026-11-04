"use client";

import { useState } from "react";
import type { SandboxSession } from "@/app/[org]/config/docActions";
import s from "@/components/catalog/catalog.module.css";
import type { CatalogResult } from "@/components/catalog/types";
import type { SessionSummary } from "@/lib/api";
import {
  DEFAULT_BRACKET_ID,
  NORMATIVE_DIRECTIONS,
  normativeBaselineSetSchema,
  type NormativeBaselineSet,
  type NormativeDirection,
  type NormativeExpectation,
  type NormativeRange,
} from "@/lib/api/config";
import { validateBaselines } from "@/lib/asd/ops";
import { baselineProblemsByClip } from "@/lib/configDocs";
import { CoreCheckPanel, DraftShell, useCoreCheck, type DraftSubmit } from "./DraftShell";
import { provenanceClass } from "./DocViews";
import { SandboxRunner } from "./SandboxRunner";
import c from "./config.module.css";
import { useText } from "@/lib/i18n/provider";


type Mutate = (fn: (set: NormativeBaselineSet) => void) => void;

/** `""` -> null; otherwise the number (NaN when not a number). */
const optNumber = (raw: string): number | null => (raw.trim() === "" ? null : Number(raw));

function OptNum({ label, value, onChange }: { label: string; value: number | null | undefined; onChange: (v: number | null) => void }) {
  const bad = typeof value === "number" && !Number.isFinite(value);
  return (
    <input
      type="number"
      step="any"
      aria-label={label}
      title={label}
      value={value === null || value === undefined || !Number.isFinite(value) ? "" : value}
      onChange={(ev) => onChange(optNumber(ev.currentTarget.value))}
      aria-invalid={bad || undefined}
      className={bad ? s.invalid : undefined}
    />
  );
}

function newExpectation(): NormativeExpectation {
  return {
    biomarker: "",
    direction: "higherBetter",
    targetAOI: null,
    byAge: { [DEFAULT_BRACKET_ID]: { expected: null } },
    citation: "",
    source: "literature",
  };
}

/** True when every number in the set is finite (JSON cannot carry NaN; the core never sees it). */
function numbersFinite(set: NormativeBaselineSet): boolean {
  const ok = (v: number | null | undefined) => v === null || v === undefined || Number.isFinite(v);
  return (
    set.ageBrackets.every((x) => Number.isFinite(x.minMonths) && Number.isFinite(x.maxMonths)) &&
    Object.values(set.videos).every(
      (v) =>
        Number.isFinite(v.weight) &&
        v.segments.every(
          (sg) =>
            Number.isFinite(sg.startSec) &&
            Number.isFinite(sg.endSec) &&
            sg.expectations.every((x) =>
              Object.values(x.byAge).every((r) => ok(r.expected) && ok(r.acceptableLow) && ok(r.acceptableHigh)),
            ),
        ),
    )
  );
}

function SegmentEditor({ clipId, index, set, mutate }: { clipId: string; index: number; set: NormativeBaselineSet; mutate: Mutate }) {
  const tx = useText();
  const b = tx.config.baselineEditor;
  const seg = set.videos[clipId]!.segments[index]!;
  const brackets = [DEFAULT_BRACKET_ID, ...set.ageBrackets.map((x) => x.id)];
  const at = (fn: (sg: typeof seg) => void) => mutate((d) => fn(d.videos[clipId]!.segments[index]!));
  const setRange = (ei: number, bracket: string, patch: Partial<NormativeRange> | null) =>
    at((sg) => {
      const byAge = sg.expectations[ei]!.byAge;
      if (patch === null) delete byAge[bracket];
      else byAge[bracket] = { ...(byAge[bracket] ?? {}), ...patch };
    });
  const setExp = (ei: number, patch: Partial<NormativeExpectation>) => at((sg) => Object.assign(sg.expectations[ei]!, patch));

  return (
    <fieldset className="panel stack" aria-label={`${b.segment} ${seg.id || index + 1}`}>
      <legend>
        {b.segment} {seg.id}
      </legend>
      <div className={s.grid}>
        <label>
          {b.segmentId}
          <input value={seg.id} onChange={(ev) => { const v = ev.currentTarget.value; at((sg) => { sg.id = v; }); }} />
        </label>
        <label>
          {b.description}
          <input value={seg.description} onChange={(ev) => { const v = ev.currentTarget.value; at((sg) => { sg.description = v; }); }} />
        </label>
        <label>
          {b.startSec}
          <OptNum label={b.startSec} value={seg.startSec} onChange={(v) => at((sg) => { sg.startSec = v ?? NaN; })} />
        </label>
        <label>
          {b.endSec}
          <OptNum label={b.endSec} value={seg.endSec} onChange={(v) => at((sg) => { sg.endSec = v ?? NaN; })} />
        </label>
      </div>

      <h4 style={{ margin: 0 }}>{b.metrics}</h4>
      {seg.expectations.map((x, ei) => (
        <div key={ei} className={`${c.metaGrid} ${provenanceClass(x.source)}`} role="group" aria-label={`${b.biomarker} ${x.biomarker || ei + 1}`}>
          <label>
            {b.biomarker}
            <input value={x.biomarker} onChange={(ev) => setExp(ei, { biomarker: ev.currentTarget.value })} />
          </label>
          <label>
            {b.targetAoi}
            <input value={x.targetAOI ?? ""} onChange={(ev) => setExp(ei, { targetAOI: ev.currentTarget.value || null })} />
          </label>
          <label>
            {b.direction}
            <select value={x.direction} onChange={(ev) => setExp(ei, { direction: ev.currentTarget.value as NormativeDirection })}>
              {NORMATIVE_DIRECTIONS.map((d) => (
                <option key={d} value={d}>{b.directions[d]}</option>
              ))}
            </select>
          </label>
          <label>
            {b.source}
            <input value={x.source} onChange={(ev) => setExp(ei, { source: ev.currentTarget.value })} />
          </label>
          <label>
            {b.citation}
            <input value={x.citation} onChange={(ev) => setExp(ei, { citation: ev.currentTarget.value })} />
          </label>
          <button type="button" className={`danger ${s.small}`} onClick={() => at((sg) => { sg.expectations.splice(ei, 1); })}>
            {b.removeMetric}
          </button>
        </div>
      ))}
      <div className={s.row}>
        <button type="button" className={s.small} onClick={() => at((sg) => { sg.expectations.push(newExpectation()); })}>
          {b.addMetric}
        </button>
      </div>

      {seg.expectations.length ? (
        <div className="table-wrap">
          <table className="data">
            <caption className={c.caption}>{b.grid(seg.id)}</caption>
            <thead>
              <tr>
                <th>{b.bracket}</th>
                {seg.expectations.map((x, ei) => (
                  <th key={ei} className={provenanceClass(x.source)}>
                    {x.targetAOI ? `${x.biomarker} · ${x.targetAOI}` : x.biomarker}
                    <span className={c.headMeta}>
                      {b.expected} · {b.low} · {b.high}
                    </span>
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {brackets.map((bracket) => (
                <tr key={bracket}>
                  <th scope="row">{bracket === DEFAULT_BRACKET_ID ? b.defaultRow : bracket}</th>
                  {seg.expectations.map((x, ei) => {
                    const r = x.byAge[bracket];
                    const cell = `${x.biomarker} ${bracket}`;
                    return (
                      <td key={ei} className={r ? provenanceClass(x.source) : c.fallback}>
                        {r ? (
                          <>
                            <div className={c.cell}>
                              <OptNum label={`${b.expected} ${cell}`} value={r.expected} onChange={(v) => setRange(ei, bracket, { expected: v })} />
                              <OptNum label={`${b.low} ${cell}`} value={r.acceptableLow} onChange={(v) => setRange(ei, bracket, { acceptableLow: v })} />
                              <OptNum label={`${b.high} ${cell}`} value={r.acceptableHigh} onChange={(v) => setRange(ei, bracket, { acceptableHigh: v })} />
                            </div>
                            <div className={c.cellActions}>
                              <button type="button" className={s.iconButton} aria-label={`${b.removeRange} ${cell}`} onClick={() => setRange(ei, bracket, null)}>
                                {b.removeRange}
                              </button>
                            </div>
                          </>
                        ) : (
                          <>
                            {b.fallback}{" "}
                            <button type="button" className={s.iconButton} aria-label={`${b.addRange} ${cell}`} onClick={() => setRange(ei, bracket, { expected: null })}>
                              {b.addRange}
                            </button>
                          </>
                        )}
                      </td>
                    );
                  })}
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      ) : null}
      <div className={s.row}>
        <button type="button" className={`danger ${s.small}`} onClick={() => mutate((d) => { d.videos[clipId]!.segments.splice(index, 1); })}>
          {b.removeSegment}
        </button>
      </div>
    </fieldset>
  );
}

/**
 * Baseline set draft editor (spec 03 §4.4): age brackets, then per clip a matrix per segment (rows = age brackets,
 * columns = metrics) with expected value and acceptable range per cell, coloured by provenance. The Rust core checks
 * coverage as you edit (asd-wasm `validateBaselines`, the backend's check) and its lines are shown per clip.
 */
export function BaselinesEditor({
  orgId,
  initial,
  fromVersion,
  preview,
  save,
}: {
  orgId: string;
  initial: NormativeBaselineSet;
  fromVersion: string | null;
  preview: {
    sessions: SessionSummary[];
    published: { label: string; profile: unknown; baselines: NormativeBaselineSet } | null;
    load: (ids: string[]) => Promise<CatalogResult<{ sessions: SandboxSession[] }>>;
  };
  save: (input: DraftSubmit<NormativeBaselineSet>) => Promise<CatalogResult<{ version: string }>>;
}) {
  const tx = useText();
  const b = tx.config.baselineEditor;
  const e = tx.config.editor;
  const [set, setSet] = useState<NormativeBaselineSet>(initial);
  const [clipId, setClipId] = useState<string | null>(Object.keys(initial.videos)[0] ?? null);
  const [newId, setNewId] = useState("");
  const [importText, setImportText] = useState("");
  const [importError, setImportError] = useState<string | null>(null);

  const mutate: Mutate = (fn) =>
    setSet((prev) => {
      const next = structuredClone(prev);
      fn(next);
      return next;
    });

  const body = numbersFinite(set) ? set : undefined;
  const check = useCoreCheck(body, checkBaselines);
  const clipIds = Object.keys(set.videos);
  const byClip = check.state === "done" ? baselineProblemsByClip(check.validation.problems, clipIds) : new Map<string, string[]>();
  const blocked = check.state === "done" ? check.validation.problems.length : 0;
  const video = clipId ? set.videos[clipId] : undefined;

  function importJson() {
    try {
      const parsed = normativeBaselineSetSchema.safeParse(JSON.parse(importText));
      if (!parsed.success) throw new Error("shape");
      setSet(parsed.data);
      setClipId(Object.keys(parsed.data.videos)[0] ?? null);
      setImportError(null);
      setImportText("");
    } catch {
      setImportError(b.importInvalid);
    }
  }

  const grouped =
    check.state === "done" && !check.validation.valid ? (
      <ul className={c.problems}>
        {[...byClip].map(([id, lines]) => (
          <li key={id || "_set"}>
            {id ? <strong>{id}</strong> : null}
            <ul className={c.problems}>
              {lines.map((l, i) => (
                <li key={i}>{l}</li>
              ))}
            </ul>
          </li>
        ))}
      </ul>
    ) : undefined;

  return (
    <DraftShell orgId={orgId} segment="baselines" fromVersion={fromVersion} body={body} blocked={blocked} save={save}>
      <CoreCheckPanel check={check} grouped={grouped} />

      <details className="panel">
        <summary>{b.settings}</summary>
        <div className={s.grid} style={{ marginTop: ".75rem" }}>
          <label>
            {b.schemaVersion}
            <input value={set.schemaVersion} onChange={(ev) => { const v = ev.currentTarget.value; mutate((d) => { d.schemaVersion = v; }); }} />
          </label>
          <label>
            {b.status}
            <input value={set.status} onChange={(ev) => { const v = ev.currentTarget.value; mutate((d) => { d.status = v; }); }} />
          </label>
          <label>
            {b.provenance}
            <input value={set.provenance ?? ""} onChange={(ev) => { const v = ev.currentTarget.value; mutate((d) => { d.provenance = v || null; }); }} />
          </label>
        </div>
        <h3>{b.brackets}</h3>
        {set.ageBrackets.map((x, i) => (
          <div key={i} className={s.row}>
            <label>
              {b.bracketId}
              <input value={x.id} onChange={(ev) => { const v = ev.currentTarget.value; mutate((d) => { d.ageBrackets[i]!.id = v; }); }} />
            </label>
            <label>
              {b.minMonths}
              <OptNum label={`${b.minMonths} ${x.id}`} value={x.minMonths} onChange={(v) => mutate((d) => { d.ageBrackets[i]!.minMonths = v ?? NaN; })} />
            </label>
            <label>
              {b.maxMonths}
              <OptNum label={`${b.maxMonths} ${x.id}`} value={x.maxMonths} onChange={(v) => mutate((d) => { d.ageBrackets[i]!.maxMonths = v ?? NaN; })} />
            </label>
            <button type="button" className={s.small} onClick={() => mutate((d) => { d.ageBrackets.splice(i, 1); })}>
              {b.removeBracket}
            </button>
          </div>
        ))}
        <button type="button" className={s.small} style={{ marginTop: ".5rem" }}
          onClick={() => mutate((d) => { d.ageBrackets.push({ id: "", minMonths: 0, maxMonths: 0 }); })}>
          {b.addBracket}
        </button>
      </details>

      <details className="panel">
        <summary>{b.importJson}</summary>
        <p className="muted">{b.importHint}</p>
        <textarea rows={6} value={importText} onChange={(ev) => setImportText(ev.currentTarget.value)} spellCheck={false} aria-label={b.importJson} />
        <div className={s.row}>
          <button type="button" className={s.small} disabled={!importText.trim()} onClick={importJson}>
            {b.importApply}
          </button>
        </div>
        {importError ? <p className={s.alert} role="alert">{importError}</p> : null}
      </details>

      <h2 className={s.section}>{b.clips}</h2>
      <p className="muted">{b.legend}</p>
      <div className={s.layout}>
        <nav aria-label={b.clipList} className="stack">
          {clipIds.length === 0 ? <p className="muted">{b.noClips}</p> : null}
          <ul className={s.clipList}>
            {clipIds.map((id) => (
              <li key={id}>
                <button type="button" className={s.clipButton} aria-current={clipId === id} onClick={() => setClipId(id)}>
                  <span>{set.videos[id]!.title || id}</span>
                  <span className={s.clipMeta}>{id}</span>
                  {byClip.has(id) ? <span className={s.errorMark}>{b.hasProblems}</span> : null}
                </button>
              </li>
            ))}
          </ul>
          <label>
            {b.newClipId}
            <input value={newId} onChange={(ev) => setNewId(ev.currentTarget.value)} />
          </label>
          <button type="button" className={s.small} disabled={!newId.trim() || clipIds.includes(newId.trim())}
            onClick={() => {
              const id = newId.trim();
              mutate((d) => { d.videos[id] = { title: "", category: "", weight: 0, segments: [] }; });
              setClipId(id);
              setNewId("");
            }}>
            {b.addClip}
          </button>
        </nav>

        {video && clipId ? (
          <section className="panel stack" aria-label={`${b.clips}: ${clipId}`}>
            <div className={s.grid}>
              <label>
                {b.title}
                <input value={video.title} onChange={(ev) => { const v = ev.currentTarget.value; mutate((d) => { d.videos[clipId]!.title = v; }); }} />
              </label>
              <label>
                {b.category}
                <input value={video.category} onChange={(ev) => { const v = ev.currentTarget.value; mutate((d) => { d.videos[clipId]!.category = v; }); }} />
              </label>
              <label>
                {b.weight}
                <OptNum label={b.weight} value={video.weight} onChange={(v) => mutate((d) => { d.videos[clipId]!.weight = v ?? NaN; })} />
              </label>
            </div>
            {byClip.get(clipId)?.length ? (
              <ul className={`${c.problems} ${s.fieldError}`}>
                {byClip.get(clipId)!.map((l, i) => (
                  <li key={i}>{l}</li>
                ))}
              </ul>
            ) : null}
            {video.segments.length === 0 ? <p className="muted">{b.noSegments}</p> : null}
            {video.segments.map((_, i) => (
              <SegmentEditor key={i} clipId={clipId} index={i} set={set} mutate={mutate} />
            ))}
            <div className={s.row}>
              <button type="button" className={s.small}
                onClick={() =>
                  mutate((d) => {
                    const segs = d.videos[clipId]!.segments;
                    segs.push({ id: `segment_${segs.length + 1}`, description: "", startSec: 0, endSec: 0, expectations: [] });
                  })
                }>
                {b.addSegment}
              </button>
              <button type="button" className={`danger ${s.small}`}
                onClick={() => {
                  mutate((d) => { delete d.videos[clipId]; });
                  setClipId(clipIds.find((x) => x !== clipId) ?? null);
                }}>
                {b.removeClip}
              </button>
            </div>
          </section>
        ) : null}
      </div>

      <details className="panel">
        <summary>{e.preview}</summary>
        <p className="muted">{e.previewIntro}</p>
        {body && preview.published ? (
          <SandboxRunner
            sessions={preview.sessions}
            initialSelected={preview.sessions.map((x) => x.id)}
            published={preview.published}
            draft={{ label: e.preview, profile: preview.published.profile, baselines: body }}
            load={preview.load}
          />
        ) : (
          <p className="muted">{tx.config.sandbox.noBaselines}</p>
        )}
      </details>
    </DraftShell>
  );
}

function checkBaselines(core: Parameters<typeof validateBaselines>[0], doc: NormativeBaselineSet) {
  return validateBaselines(core, doc);
}
