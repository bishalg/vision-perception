import { en } from "@/messages/en";
import type { ClipScore } from "@/lib/api";
import { formatNumber } from "@/lib/format";

const c = en.detail.clips;

export function ClipsTable({ clips }: { clips: ClipScore[] }) {
  if (clips.length === 0) return <p className="muted">{c.empty}</p>;
  const sorted = [...clips].sort((a, b) => a.order - b.order);
  return (
    <div className="table-wrap">
      <table className="data responsive">
        <thead>
          <tr>
            <th scope="col">{c.clip}</th>
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
          {sorted.map((clip) => (
            <tr key={`${clip.order}-${clip.clip_id}`}>
              <td data-label={c.clip}>{clip.clip_id}</td>
              <td data-label={c.metric}>{clip.metric ?? "-"}</td>
              <td data-label={c.expected} className="num" title={clip.baseline_ref ?? undefined}>
                {formatNumber(clip.expected, 2)}
              </td>
              <td data-label={c.recorded} className="num">{formatNumber(clip.recorded, 2)}</td>
              <td data-label={c.deviation} className="num">{formatNumber(clip.deviation, 2)}</td>
              <td data-label={c.status}>
                <span className={`badge badge-clip-${clip.status}`}>{en.clipStatus[clip.status]}</span>
              </td>
              <td data-label={c.weight} className="num">{formatNumber(clip.weight, 2)}</td>
              <td data-label={c.score} className="num">{formatNumber(clip.score, 0)}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
