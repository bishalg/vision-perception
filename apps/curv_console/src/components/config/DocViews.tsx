"use client";

/**
 * Read-only views of a scoring profile and a baseline set, and the diff between two versions (WP-C.5).
 */
import s from "@/components/catalog/catalog.module.css";
import { DEFAULT_BRACKET_ID, type NormativeBaselineSet, type NormativeExpectation, type ScoringProfile } from "@/lib/api/config";
import { formatValue, groupBaselineChanges, type LeafChange } from "@/lib/configDocs";
import c from "./config.module.css";
import { useText } from "@/lib/i18n/provider";


export const PROFILE_FIELDS = {
  flag: ["greenFloor", "yellowFloor", "lowConfidenceShare", "atypicalShareForRed"],
  qualityGate: ["minDistanceCm", "maxDistanceCm", "maxYawRad", "maxPitchRad", "minConfidence", "blinkThreshold"],
} as const;

export function ScoringProfileView({ profile }: { profile: ScoringProfile }) {
  const tx = useText();
  const p = tx.config.profile;
  return (
    <div className={c.columns}>
      <section className="panel" aria-label={p.flag}>
        <h2>{p.flag}</h2>
        <dl className="facts">
          {PROFILE_FIELDS.flag.map((k) => (
            <div key={k}><dt>{p[k]}</dt><dd>{profile.flag[k]}</dd></div>
          ))}
          <div><dt>{p.lowConfidenceValidRatio}</dt><dd>{profile.lowConfidenceValidRatio}</dd></div>
          <div><dt>{p.schemaVersion}</dt><dd>{profile.schemaVersion}</dd></div>
        </dl>
      </section>
      <section className="panel" aria-label={p.qualityGate}>
        <h2>{p.qualityGate}</h2>
        <dl className="facts">
          {PROFILE_FIELDS.qualityGate.map((k) => (
            <div key={k}><dt>{p[k]}</dt><dd>{profile.qualityGate[k]}</dd></div>
          ))}
        </dl>
      </section>
      {profile.trackingQuality ? (
        <details className="panel">
          <summary>{p.tracking}</summary>
          <pre className={s.pre}>{JSON.stringify(profile.trackingQuality, null, 2)}</pre>
        </details>
      ) : null}
    </div>
  );
}

/** CSS class for an expectation's provenance (`source`). */
export function provenanceClass(source: string): string {
  if (source.startsWith("own")) return c.provOwn!;
  if (source === "interpolated") return c.provInterpolated!;
  return c.provLiterature!;
}

const label = (e: NormativeExpectation) => (e.targetAOI ? `${e.biomarker} · ${e.targetAOI}` : e.biomarker);
const num = (v: number | null | undefined) => (v === null || v === undefined ? "—" : String(v));

export function BaselinesView({ set }: { set: NormativeBaselineSet }) {
  const tx = useText();
  const b = tx.config.baselineEditor;
  const rows = [DEFAULT_BRACKET_ID, ...set.ageBrackets.map((x) => x.id)];
  return (
    <div className="stack">
      <p className="muted">{b.legend}</p>
      {Object.entries(set.videos).map(([id, video]) => (
        <section key={id} className="panel stack" aria-label={id}>
          <h2 style={{ margin: 0 }}>{video.title || id}</h2>
          <p className="muted" style={{ margin: 0 }}>
            {id} · {video.category} · {b.weight} {video.weight}
          </p>
          {video.segments.map((seg) => (
            <div key={seg.id} className="table-wrap">
              <table className="data">
                <caption className={c.caption}>
                  {b.grid(seg.id)} ({b.segmentSpan(seg.startSec, seg.endSec)})
                </caption>
                <thead>
                  <tr>
                    <th>{b.bracket}</th>
                    {seg.expectations.map((e, i) => (
                      <th key={i} className={provenanceClass(e.source)} title={`${e.source}: ${e.citation}`}>
                        {label(e)}
                        <span className={c.headMeta}>{b.directions[e.direction] ?? e.direction} · {e.source}</span>
                      </th>
                    ))}
                  </tr>
                </thead>
                <tbody>
                  {rows.map((bracket) => (
                    <tr key={bracket}>
                      <th scope="row">{bracket === DEFAULT_BRACKET_ID ? b.defaultRow : bracket}</th>
                      {seg.expectations.map((e, i) => {
                        const r = e.byAge[bracket];
                        return (
                          <td key={i} className={r ? provenanceClass(e.source) : c.fallback}>
                            {r ? `${num(r.expected)} [${num(r.acceptableLow)}, ${num(r.acceptableHigh)}]` : b.fallback}
                          </td>
                        );
                      })}
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          ))}
        </section>
      ))}
    </div>
  );
}

function ChangeTable({ changes }: { changes: LeafChange[] }) {
  const tx = useText();
  const t = tx.config;
  return (
    <div className="table-wrap">
      <table className="data">
        <thead>
          <tr><th>{t.diff.field}</th><th>{t.diff.before}</th><th>{t.diff.after}</th></tr>
        </thead>
        <tbody>
          {changes.map((x) => (
            <tr key={x.path}>
              <td>{x.path}</td>
              <td>{formatValue(x.before)}</td>
              <td>{formatValue(x.after)}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

export function DocDiffView({
  from,
  to,
  changes,
  groupByClip,
}: {
  from: string;
  to: string;
  changes: LeafChange[];
  groupByClip?: boolean;
}) {
  const tx = useText();
  const t = tx.config;
  const b = tx.config.baselineEditor;
  const title = t.diff.title(from, to);
  return (
    <section className={`panel ${s.diff}`} aria-label={title}>
      <h2>{title}</h2>
      {!changes.length ? <p className="muted">{t.diff.none}</p> : null}
      {changes.length && groupByClip
        ? [...groupBaselineChanges(changes)].map(([clip, list]) => (
            <div key={clip || "_set"}>
              <h3>{clip || b.settings}</h3>
              <ChangeTable changes={list} />
            </div>
          ))
        : changes.length
          ? <ChangeTable changes={changes} />
          : null}
    </section>
  );
}
