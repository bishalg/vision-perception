import type { Dist } from "@/lib/api/research";
import type { BaselineRange } from "@/lib/research/baselines";
import { formatNumber } from "@/lib/format";
import { researchText, type ResearchMessages } from "@/messages/research";
import r from "./research.module.css";

const W = 320;
const H = 140;
const PAD = { l: 30, r: 8, t: 8, b: 22 };

/**
 * One distribution as inline SVG bars (RUN-4). The counts come from the backend, already small-cell suppressed: a
 * suppressed distribution (no histogram) shows the "n < k" message and no bars, never zeros. The published baseline
 * range of the metric for the age band, when the caller has one, is shaded behind the bars.
 */
export function Histogram({
  label,
  dist,
  k,
  baseline = null,
  text = researchText,
}: {
  label: string;
  dist: Dist;
  k: number;
  baseline?: BaselineRange | null;
  text?: ResearchMessages;
}) {
  const hist = dist.hist;
  if (dist.suppressed || !hist || hist.counts.length === 0 || hist.edges.length !== hist.counts.length + 1) {
    return (
      <figure className={r.chart} data-testid="histogram-suppressed">
        <figcaption>{label}</figcaption>
        <p className="muted">{text.suppressedCell(k)}</p>
      </figure>
    );
  }
  const t = text.charts;
  const lo = hist.edges[0]!;
  const hi = hist.edges[hist.edges.length - 1]!;
  const span = hi - lo || 1;
  const max = Math.max(1, ...hist.counts);
  const x = (v: number) => PAD.l + ((Math.min(Math.max(v, lo), hi) - lo) / span) * (W - PAD.l - PAD.r);
  const y = (c: number) => H - PAD.b - (c / max) * (H - PAD.t - PAD.b);
  const shade = baseline && baseline.high >= lo && baseline.low <= hi ? baseline : null;
  return (
    <figure className={r.chart}>
      <figcaption>{label}</figcaption>
      <svg className={r.histogram} viewBox={`0 0 ${W} ${H}`} role="img" aria-label={t.histogram(label)}>
        {shade ? (
          <rect
            data-testid="baseline-range"
            className={r.baselineBand}
            x={x(shade.low)}
            y={PAD.t}
            width={Math.max(x(shade.high) - x(shade.low), 1)}
            height={H - PAD.t - PAD.b}
          />
        ) : null}
        <line className={r.axis} x1={PAD.l} y1={H - PAD.b} x2={W - PAD.r} y2={H - PAD.b} />
        {hist.counts.map((count, i) => {
          const from = hist.edges[i]!;
          const to = hist.edges[i + 1]!;
          return (
            <rect key={i} data-testid="hist-bar" className={r.histBar} x={x(from) + 1} y={y(count)} width={Math.max(x(to) - x(from) - 2, 1)} height={H - PAD.b - y(count)}>
              <title>{t.bin(formatNumber(from, 2), formatNumber(to, 2), count)}</title>
            </rect>
          );
        })}
        <text className={r.axisText} x={PAD.l} y={H - 6} textAnchor="start">{formatNumber(lo, 2)}</text>
        <text className={r.axisText} x={W - PAD.r} y={H - 6} textAnchor="end">{formatNumber(hi, 2)}</text>
        <text className={r.axisText} x={PAD.l - 4} y={PAD.t + 8} textAnchor="end">{max}</text>
      </svg>
      <p className="muted">{shade ? t.baseline(formatNumber(shade.low, 2), formatNumber(shade.high, 2)) : t.noBaseline}</p>
    </figure>
  );
}
