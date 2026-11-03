import { formatValue } from "@/lib/research/registry";
import { researchText, type ResearchMessages } from "@/messages/research";
import r from "./research.module.css";

export interface CurvePoint {
  x: number;
  y: number | null;
}

const W = 320;
const H = 160;
const PAD = 28;

/**
 * Inline SVG line of the composite score against one swept knob (RUI-4), with the published value marked. Draws the
 * numbers the Rust core returned, scaled to the plot; a step without a score leaves a gap.
 */
export function SweepCurve({
  points,
  published,
  label,
  text = researchText,
}: {
  points: CurvePoint[];
  published: number;
  label: string;
  text?: ResearchMessages;
}) {
  const xs = points.map((p) => p.x);
  const ys = points.flatMap((p) => (p.y === null ? [] : [p.y]));
  if (!points.length) return null;
  const [x0, x1] = [Math.min(...xs), Math.max(...xs)];
  const [y0, y1] = ys.length ? [Math.min(...ys), Math.max(...ys)] : [0, 1];
  const sx = (x: number) => PAD + (x1 === x0 ? 0.5 : (x - x0) / (x1 - x0)) * (W - 2 * PAD);
  const sy = (y: number) => H - PAD - (y1 === y0 ? 0.5 : (y - y0) / (y1 - y0)) * (H - 2 * PAD);
  const segments: string[] = [];
  let current: string[] = [];
  for (const p of points) {
    if (p.y === null) {
      if (current.length) segments.push(current.join(" "));
      current = [];
    } else current.push(`${sx(p.x).toFixed(1)},${sy(p.y).toFixed(1)}`);
  }
  if (current.length) segments.push(current.join(" "));
  const mark = published >= x0 && published <= x1 ? sx(published) : null;
  return (
    <svg className={r.curve} viewBox={`0 0 ${W} ${H}`} role="img" aria-label={text.sweep.curve(label)} data-testid="sweep-curve">
      <line className={r.axis} x1={PAD} y1={H - PAD} x2={W - PAD} y2={H - PAD} />
      <line className={r.axis} x1={PAD} y1={PAD} x2={PAD} y2={H - PAD} />
      {mark !== null ? (
        <line className={r.markLine} x1={mark} y1={PAD} x2={mark} y2={H - PAD}>
          <title>{text.sweep.publishedMarker}</title>
        </line>
      ) : null}
      {segments.map((pts, i) => <polyline key={i} className={r.curveLine} points={pts} />)}
      {points.map((p) => (p.y === null ? null : <circle key={p.x} className={r.curveDot} cx={sx(p.x)} cy={sy(p.y)} r={2.5}><title>{`${formatValue(p.x)}: ${formatValue(p.y)}`}</title></circle>))}
      <text className={r.axisText} x={PAD} y={H - 10}>{formatValue(x0)}</text>
      <text className={r.axisText} x={W - PAD} y={H - 10} textAnchor="end">{formatValue(x1)}</text>
      <text className={r.axisText} x={4} y={H - PAD}>{formatValue(y0)}</text>
      <text className={r.axisText} x={4} y={PAD + 4}>{formatValue(y1)}</text>
    </svg>
  );
}
