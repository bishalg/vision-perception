import type { Dist } from "@/lib/api/research";
import { formatNumber } from "@/lib/format";
import type { ResearchMessages } from "@/messages/research";
import r from "./research.module.css";

export interface DeltaRow {
  label: string;
  delta: Dist;
}

const W = 360;
const ROW = 26;
const PAD = { l: 96, r: 12, t: 8, b: 20 };

/**
 * The change (draft minus published) of one measure, one bar per age band, with the 95% interval of the mean as a
 * whisker (RUN-4). A suppressed band has no mean: it shows the "n < k" message and no bar. Values are drawn as sent.
 */
export function DeltaBars({
  label,
  rows,
  k,
  text,
}: {
  label: string;
  rows: readonly DeltaRow[];
  k: number;
  text: ResearchMessages;
}) {
  const shown = rows.filter((row) => !row.delta.suppressed && row.delta.mean != null);
  const hidden = rows.filter((row) => !shown.includes(row));
  const extent = Math.max(
    1e-9,
    ...shown.flatMap((row) => [Math.abs(row.delta.mean ?? 0), Math.abs(row.delta.ci95_mean?.[0] ?? 0), Math.abs(row.delta.ci95_mean?.[1] ?? 0)]),
  );
  const zero = PAD.l + (W - PAD.l - PAD.r) / 2;
  const x = (v: number) => zero + (v / extent) * ((W - PAD.l - PAD.r) / 2);
  const height = PAD.t + shown.length * ROW + PAD.b;
  return (
    <figure className={r.chart}>
      {shown.length > 0 ? (
        <svg className={r.deltaBars} viewBox={`0 0 ${W} ${height}`} role="img" aria-label={text.charts.deltaBars(label)}>
          <line className={r.axis} x1={zero} y1={PAD.t} x2={zero} y2={height - PAD.b} />
          {shown.map((row, i) => {
            const mean = row.delta.mean!;
            const yMid = PAD.t + i * ROW + ROW / 2;
            const ci = row.delta.ci95_mean;
            return (
              <g key={row.label} data-testid="delta-bar">
                <text className={r.axisText} x={PAD.l - 6} y={yMid + 3} textAnchor="end">{row.label}</text>
                <rect className={mean >= 0 ? r.deltaUp : r.deltaDown} x={Math.min(zero, x(mean))} y={yMid - 6} width={Math.abs(x(mean) - zero)} height={12}>
                  <title>{row.label}: {formatNumber(mean, 3)}</title>
                </rect>
                {ci ? <line className={r.whisker} x1={x(ci[0])} y1={yMid} x2={x(ci[1])} y2={yMid} /> : null}
              </g>
            );
          })}
          <text className={r.axisText} x={zero} y={height - 6} textAnchor="middle">{formatNumber(0, 0)}</text>
        </svg>
      ) : (
        <p className="muted">{text.charts.noData}</p>
      )}
      {shown.length > 0 ? <p className="muted">{text.charts.interval}</p> : null}
      {hidden.length > 0 ? (
        <ul>
          {hidden.map((row) => (
            <li key={row.label} data-testid="delta-suppressed">{row.label}: {text.suppressedCell(k)}</li>
          ))}
        </ul>
      ) : null}
    </figure>
  );
}
