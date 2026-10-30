import { en } from "@/messages/en";
import type { ClipScore, ClipStatus, MetricRow } from "@/lib/api";
import { formatNumber } from "@/lib/format";

const c = en.detail.clips;

function StatusBadge({ status }: { status: ClipStatus | null | undefined }) {
  if (!status) return <span className="muted">{c.notScored}</span>;
  return <span className={`badge badge-clip-${status}`}>{en.clipStatus[status]}</span>;
}

function range(row: MetricRow): string | undefined {
  const { acceptable_low: lo, acceptable_high: hi } = row;
  if (lo == null && hi == null) return undefined;
  return `${formatNumber(lo, 2)} to ${formatNumber(hi, 2)}`;
}

/**
 * One line per metric row of each clip (CON-19), in play order. A clip without rows (status null) gets a single
 * "Not scored" line. All values are shown exactly as the backend computed them; nothing is derived here.
 */
export function ClipsTable({ clips }: { clips: ClipScore[] }) {
  if (clips.length === 0) return <p className="muted">{c.empty}</p>;
  const sorted = [...clips].sort((a, b) => a.order - b.order);
  return (
    <div className="table-wrap">
      <table className="data responsive">
        <thead>
          <tr>
            <th scope="col">{c.clip}</th>
            <th scope="col">{c.segment}</th>
            <th scope="col">{c.metric}</th>
            <th scope="col" className="num">{c.expected}</th>
            <th scope="col" className="num">{c.recorded}</th>
            <th scope="col" className="num">{c.deviation}</th>
            <th scope="col">{c.status}</th>
            <th scope="col" className="num">{c.weight}</th>
            <th scope="col" className="num">{c.score}</th>
          </tr>
        </thead>
        <tbody>
          {sorted.flatMap((clip) =>
            clip.rows.length === 0
              ? [
                  <tr key={`${clip.order}-${clip.clip_id}`}>
                    <td data-label={c.clip}>{clip.clip_id}</td>
                    <td data-label={c.segment}>-</td>
                    <td data-label={c.metric}>{clip.metric ?? "-"}</td>
                    <td data-label={c.expected} className="num" title={clip.baseline_ref ?? undefined}>{formatNumber(clip.expected, 2)}</td>
                    <td data-label={c.recorded} className="num">{formatNumber(clip.recorded, 2)}</td>
                    <td data-label={c.deviation} className="num">{formatNumber(clip.deviation, 2)}</td>
                    <td data-label={c.status}><StatusBadge status={clip.status} /></td>
                    <td data-label={c.weight} className="num">{formatNumber(clip.weight, 2)}</td>
                    <td data-label={c.score} className="num">{formatNumber(clip.score, 2)}</td>
                  </tr>,
                ]
              : clip.rows.map((row, i) => (
                  <tr key={`${clip.order}-${clip.clip_id}-${i}`}>
                    <td data-label={c.clip}>{clip.clip_id}</td>
                    <td data-label={c.segment}>{row.segment_id}</td>
                    <td data-label={c.metric} title={row.source}>
                      {row.metric}
                      {row.imputed ? <span className="muted"> ({c.imputed})</span> : null}
                    </td>
                    <td data-label={c.expected} className="num" title={range(row)}>{formatNumber(row.expected, 2)}</td>
                    <td data-label={c.recorded} className="num">{formatNumber(row.recorded, 2)}</td>
                    <td data-label={c.deviation} className="num">{formatNumber(row.deviation, 2)}</td>
                    <td data-label={c.status}><StatusBadge status={row.status} /></td>
                    <td data-label={c.weight} className="num">{formatNumber(clip.weight, 2)}</td>
                    <td data-label={c.score} className="num">{formatNumber(row.score, 2)}</td>
                  </tr>
                )),
          )}
        </tbody>
      </table>
    </div>
  );
}
