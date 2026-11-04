import type { BandSummary, RunSummary } from "@/lib/api/research";
import { baselineRange } from "@/lib/research/baselines";
import type { ResearchMessages } from "@/messages/research";
import { DeltaBars } from "./DeltaBars";
import { Histogram } from "./Histogram";
import r from "./research.module.css";
import { TransitionMatrix } from "./TransitionMatrix";

type FlagName = keyof ResearchMessages["preview"]["flagLabel"];
const FLAGS: FlagName[] = ["green", "yellow", "red", "unreliable"];

/**
 * The results of a finished run (RUN-4): counts, flag counts and transitions, then the composite and each metric per
 * age band as histograms (with the published baseline range shaded when the effective baselines state one) and the
 * draft-minus-published change as bars. Everything is drawn as the backend sent it; a suppressed cell shows "n < k".
 */
export function RunResults({
  summary,
  baselines,
  membershipDrift,
  text,
}: {
  summary: RunSummary;
  /** `normative_baselines.body` of the effective config; `unknown` is read tolerantly, or null when unavailable. */
  baselines: unknown;
  membershipDrift?: { added: number; removed: number } | null;
  text: ResearchMessages;
}) {
  const t = text.runs.results;
  const k = summary.small_cell_k;
  const flagLabel = (f: string) => text.preview.flagLabel[f as FlagName] ?? f;
  const cell = (n: number | null | undefined) => (n == null ? text.suppressedCell(k) : String(n));
  const byLevel = `${text.level.frames}: ${summary.counts.by_level.frames}, ${text.level.score}: ${summary.counts.by_level.score}`;
  const bands = [summary.overall, ...summary.by_age_band];
  const bandRows = summary.by_age_band.map((b) => ({ label: b.band, delta: b.composite.delta }));
  const metricIds = [...new Set(bands.flatMap((b) => Object.keys(b.metrics)))];
  return (
    <section className="stack" aria-label={t.title}>
      <h2>{t.title}</h2>
      <section className="panel stack" aria-label={t.sessions}>
        <dl className="facts">
          <div><dt>{t.sessions}</dt><dd>{summary.counts.sessions}</dd></div>
          <div><dt>{t.byLevel}</dt><dd>{byLevel}</dd></div>
          {Object.entries(summary.counts.excluded).map(([reason, n]) => (
            <div key={reason}><dt>{t.excluded}: {reason}</dt><dd>{n}</dd></div>
          ))}
          {Object.entries(summary.counts.params_not_applied).map(([id, n]) => (
            <div key={id}><dt>{t.paramsNotApplied}: {id}</dt><dd>{n}</dd></div>
          ))}
          {summary.counts.suppressed_cells != null ? <div><dt>{t.suppressedCells}</dt><dd>{summary.counts.suppressed_cells}</dd></div> : null}
        </dl>
        {summary.includes_synthetic ? <p className="muted">{t.synthetic}</p> : null}
        {membershipDrift ? <p className="muted">{t.drift(membershipDrift.added, membershipDrift.removed)}</p> : null}
      </section>

      <section className="panel stack" aria-label={t.flagCounts}>
        <h3>{t.flagCounts}</h3>
        <div className="table-wrap">
          <table className="data" aria-label={t.flagCounts}>
            <thead>
              <tr>
                <th scope="col">{t.published} / {t.draft}</th>
                {FLAGS.map((f) => <th key={f} scope="col">{flagLabel(f)}</th>)}
              </tr>
            </thead>
            <tbody>
              {(["published", "draft"] as const).map((side) => (
                <tr key={side}>
                  <th scope="row">{t[side]}</th>
                  {FLAGS.map((f) => <td key={f} className="num" data-testid="flag-count">{cell(summary.flags[side][f])}</td>)}
                </tr>
              ))}
            </tbody>
          </table>
        </div>
        <TransitionMatrix caption={t.flagTransitions} transitions={summary.flags.transitions} k={k} stateLabel={flagLabel} text={text} />
        <TransitionMatrix caption={t.qualityTransitions} transitions={summary.quality.transitions} k={k} text={text} />
      </section>

      {bandRows.length > 0 ? (
        <section className="panel stack" aria-label={t.deltaTitle}>
          <h3>{t.deltaTitle}</h3>
          <DeltaBars label={t.composite} rows={bandRows} k={k} text={text} />
        </section>
      ) : null}

      {bands.map((band) => (
        <BandCharts key={band.band} band={band} k={k} metricIds={metricIds} baselines={baselines} text={text} />
      ))}
    </section>
  );
}

function BandCharts({
  band,
  k,
  metricIds,
  baselines,
  text,
}: {
  band: BandSummary;
  k: number;
  metricIds: string[];
  baselines: unknown;
  text: ResearchMessages;
}) {
  const t = text.runs.results;
  const title = band.band === "overall" ? t.overall : t.band(band.band);
  if (band.suppressed) {
    return (
      <section className="panel stack" aria-label={title} data-band={band.band}>
        <h3>{title}</h3>
        <p className="muted">{text.suppressedCell(k)}</p>
      </section>
    );
  }
  return (
    <section className="panel stack" aria-label={title} data-band={band.band}>
      <h3>{title}</h3>
      <div className={r.charts}>
        <Histogram label={`${t.composite}: ${t.published}`} dist={band.composite.published} k={k} text={text} />
        <Histogram label={`${t.composite}: ${t.draft}`} dist={band.composite.draft} k={k} text={text} />
        {metricIds.filter((id) => band.metrics[id]).flatMap((id) => {
          const m = band.metrics[id]!;
          const range = baselineRange(baselines, id, band.band);
          return [
            <Histogram key={`${id}-p`} label={`${t.metric(id)}: ${t.published}`} dist={m.published} k={k} baseline={range} text={text} />,
            <Histogram key={`${id}-d`} label={`${t.metric(id)}: ${t.draft}`} dist={m.draft} k={k} baseline={range} text={text} />,
          ];
        })}
      </div>
    </section>
  );
}
