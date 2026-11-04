import { formatPercent } from "@/lib/format";
import { formatValue } from "@/lib/research/registry";
import { gridKey, gridLayout, isPublishedCell, orderedCells, shadeStep, type SweepCell, type SweepSummary } from "@/lib/research/run-sweep";
import type { ResearchMessages } from "@/messages/research";
import type { Locale } from "@/lib/i18n/types";
import { paramText } from "@/lib/research/paramText";
import r from "./research.module.css";
import { SweepCurve } from "./SweepCurve";

/** The six shade classes of the heatmap scale (light to dark). */
const HEAT = [r.heat0, r.heat1, r.heat2, r.heat3, r.heat4, r.heat5];
const CELL = 12;
const PAD = { l: 44, r: 8, t: 8, b: 28 };

type Quantity = { label: string; value: (c: SweepCell) => number | null | undefined; format: (v: number) => string };

/**
 * The `param_sweep` section of a run page (SWP-3, RUI-6): one axis draws curves of the mean composite, the green share
 * and each chosen metric against the knob with the published value marked; two axes draw heatmaps (one shade step per
 * cell, the published cell outlined, hidden cells grey). Every cell is also in a table. All numbers are the Rust job's;
 * a suppressed cell (PRV-1) shows "n < k", never a number.
 */
export function SweepPanel({ sweep, k, text, locale = "en" }: { sweep: SweepSummary; k: number; text: ResearchMessages; locale?: Locale }) {
  const t = text.analysis;
  const label = (id: string) => paramText(locale, id).label;
  const cells = orderedCells(sweep);
  const metricIds = [...new Set(sweep.cells.flatMap((c) => Object.keys(c.metrics)))];
  const quantities: Quantity[] = [
    { label: t.compositeMean, value: (c) => c.composite.mean, format: formatValue },
    { label: t.greenShare, value: (c) => c.flag_rate?.green, format: formatPercent },
    ...metricIds.map((id): Quantity => ({ label: t.metricMean(id), value: (c) => c.metrics[id]?.mean, format: formatValue })),
  ];
  const published = sweep.published_values ?? [];
  return (
    <section className="panel stack" aria-label={t.sweepTitle} data-testid="sweep-panel">
      <h2>{t.sweepTitle}</h2>
      <p className="muted">{t.sweepIntro(sweep.cells.length)}</p>
      <ul>
        {sweep.axes.map((a, i) => (
          <li key={a.param_id}>{t.axis(i + 1, label(a.param_id), a.param_id)}</li>
        ))}
      </ul>
      {published.length ? <p>{t.publishedValue(published.map(formatValue).join(", "))}</p> : null}
      {sweep.axes.length === 1 ? (
        <div className={r.charts}>
          {quantities.map((q) => (
            <figure key={q.label} className={r.chart}>
              <figcaption>{q.label}</figcaption>
              <SweepCurve
                label={q.label}
                published={published[0] ?? NaN}
                points={cells.map((c) => ({ x: c.values[0] ?? 0, y: c.n == null ? null : (q.value(c) ?? null) }))}
                text={text}
              />
            </figure>
          ))}
        </div>
      ) : (
        <TwoAxis sweep={sweep} k={k} quantities={quantities.slice(0, 2)} xLabel={label(sweep.axes[0]!.param_id)} yLabel={label(sweep.axes[1]!.param_id)} text={text} />
      )}
      <details>
        <summary>{t.showTable}</summary>
        <SweepTable sweep={sweep} cells={cells} metricIds={metricIds} k={k} text={text} />
      </details>
    </section>
  );
}

function TwoAxis({
  sweep,
  k,
  quantities,
  xLabel,
  yLabel,
  text,
}: {
  sweep: SweepSummary;
  k: number;
  quantities: Quantity[];
  xLabel: string;
  yLabel: string;
  text: ResearchMessages;
}) {
  const t = text.analysis;
  const grid = gridLayout(sweep);
  const W = PAD.l + grid.xs.length * CELL + PAD.r;
  const H = PAD.t + grid.ys.length * CELL + PAD.b;
  const x = (i: number) => PAD.l + i * CELL;
  // Larger values of the second axis at the top.
  const y = (j: number) => PAD.t + (grid.ys.length - 1 - j) * CELL;
  const published = sweep.published_values ?? [];
  const pubX = grid.xs.indexOf(published[0] ?? NaN);
  const pubY = grid.ys.indexOf(published[1] ?? NaN);
  return (
    <>
      {grid.offGrid ? <p className="muted">{t.offGrid(grid.offGrid.values.map(formatValue).join(", "))}</p> : null}
      <div className={r.charts}>
        {quantities.map((q) => {
          const values = [...grid.at.values()].flatMap((c) => {
            const v = c.n == null ? null : q.value(c);
            return v == null ? [] : [v];
          });
          const lo = values.length ? Math.min(...values) : 0;
          const hi = values.length ? Math.max(...values) : 0;
          return (
            <figure key={q.label} className={r.chart}>
              <figcaption>{q.label}</figcaption>
              <svg className={r.heatmap} viewBox={`0 0 ${W} ${H}`} role="img" aria-label={t.heatmap(q.label, xLabel, yLabel)} data-testid="sweep-heatmap">
                {grid.xs.flatMap((xv, i) =>
                  grid.ys.map((yv, j) => {
                    const cell = grid.at.get(gridKey(xv, yv));
                    if (!cell) return null;
                    const v = cell.n == null ? null : q.value(cell);
                    const cls = v == null ? r.heatSuppressed : HEAT[shadeStep(v, lo, hi, HEAT.length)];
                    const n = cell.n == null ? text.suppressedCell(k) : String(cell.n);
                    return (
                      <rect key={`${i}-${j}`} className={cls} x={x(i)} y={y(j)} width={CELL} height={CELL} data-testid="sweep-cell">
                        <title>{t.cellTitle(cell.values.map(formatValue).join(", "), n, v == null ? text.suppressedCell(k) : q.format(v))}</title>
                      </rect>
                    );
                  }),
                )}
                {pubX >= 0 && pubY >= 0 ? (
                  <rect className={r.heatPublished} x={x(pubX)} y={y(pubY)} width={CELL} height={CELL} data-testid="sweep-published">
                    <title>{t.publishedMarker}</title>
                  </rect>
                ) : null}
                <text className={r.axisText} x={PAD.l} y={H - 14}>{formatValue(grid.xs[0] ?? 0)}</text>
                <text className={r.axisText} x={W - PAD.r} y={H - 14} textAnchor="end">{formatValue(grid.xs[grid.xs.length - 1] ?? 0)}</text>
                <text className={r.axisText} x={PAD.l + (W - PAD.l - PAD.r) / 2} y={H - 2} textAnchor="middle">{xLabel}</text>
                <text className={r.axisText} x={4} y={H - PAD.b}>{formatValue(grid.ys[0] ?? 0)}</text>
                <text className={r.axisText} x={4} y={PAD.t + 8}>{formatValue(grid.ys[grid.ys.length - 1] ?? 0)}</text>
              </svg>
              <p className={r.heatLegend}>
                <span>{t.heatLegend(values.length ? q.format(lo) : "-", values.length ? q.format(hi) : "-")}</span>
                <span>{t.heatSuppressed(k)}</span>
                <span>{t.publishedOutline}</span>
              </p>
            </figure>
          );
        })}
      </div>
    </>
  );
}

function SweepTable({
  sweep,
  cells,
  metricIds,
  k,
  text,
}: {
  sweep: SweepSummary;
  cells: SweepCell[];
  metricIds: string[];
  k: number;
  text: ResearchMessages;
}) {
  const t = text.analysis;
  const hidden = text.suppressedCell(k);
  const rate = (c: SweepCell, f: "green" | "yellow" | "red" | "unreliable") => (c.n == null || !c.flag_rate ? hidden : formatPercent(c.flag_rate[f]));
  const num = (c: SweepCell, v: number | null | undefined) => (c.n == null ? hidden : v == null ? "-" : formatValue(v));
  return (
    <div className="table-wrap">
      <table className="data" aria-label={t.tableTitle}>
        <thead>
          <tr>
            <th scope="col">{t.cols.values}</th>
            <th scope="col">{t.cols.n}</th>
            <th scope="col">{t.cols.green}</th>
            <th scope="col">{t.cols.yellow}</th>
            <th scope="col">{t.cols.red}</th>
            <th scope="col">{t.cols.unreliable}</th>
            <th scope="col">{t.cols.compositeMean}</th>
            <th scope="col">{t.cols.compositeMedian}</th>
            <th scope="col">{t.cols.changed}</th>
            {metricIds.map((id) => <th key={id} scope="col">{t.metricMean(id)}</th>)}
          </tr>
        </thead>
        <tbody>
          {cells.map((c) => {
            const pub = isPublishedCell(sweep, c);
            const rowLabel = `${c.values.map(formatValue).join(", ")}${pub ? ` (${t.publishedRow})` : ""}`;
            return (
              <tr key={c.values.join("|")} data-published={pub || undefined}>
                <th scope="row">{rowLabel}</th>
                <td className="num">{c.n == null ? hidden : c.n}</td>
                <td className="num">{rate(c, "green")}</td>
                <td className="num">{rate(c, "yellow")}</td>
                <td className="num">{rate(c, "red")}</td>
                <td className="num">{rate(c, "unreliable")}</td>
                <td className="num">{num(c, c.composite.mean)}</td>
                <td className="num">{num(c, c.composite.median)}</td>
                <td className="num">{c.n == null || c.changed_vs_published == null ? hidden : c.changed_vs_published}</td>
                {metricIds.map((id) => <td key={id} className="num">{num(c, c.metrics[id]?.mean)}</td>)}
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}
