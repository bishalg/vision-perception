import { en } from "@/messages/en";
import type { SessionTrackingQuality } from "@/lib/api";

const t = en.detail.tracking.drift;

const W = 640;
const H = 220;
const PAD = { l: 44, r: 12, t: 12, b: 34 };

/**
 * Gaze offset at each inter-clip check over session time. Values are plotted as sent by the API; the
 * warn/hard lines are drawn only when the caller passes the profile thresholds (the payload carries none
 * itself). A missed check has no offset: it is marked on the axis and not joined into the line.
 */
export function DriftChart({
  checks,
  warnDeg,
  hardDeg,
}: {
  checks: SessionTrackingQuality["drift_checks"];
  warnDeg?: number | null;
  hardDeg?: number | null;
}) {
  if (checks.length === 0) return <p className="muted">{t.empty}</p>;
  const lines: { key: string; label: string; deg: number }[] = [];
  if (warnDeg != null) lines.push({ key: "warn", label: t.warn, deg: warnDeg });
  if (hardDeg != null) lines.push({ key: "hard", label: t.hard, deg: hardDeg });
  const maxSec = Math.max(...checks.map((c) => c.at_sec), 1);
  const maxDeg = Math.max(1, ...checks.map((c) => c.offset_deg ?? 0), ...lines.map((l) => l.deg)) * 1.1;
  const x = (sec: number) => PAD.l + (sec / maxSec) * (W - PAD.l - PAD.r);
  const y = (deg: number) => H - PAD.b - (deg / maxDeg) * (H - PAD.t - PAD.b);
  const measured = checks.filter((c) => c.offset_deg != null);
  const path = measured.map((c, k) => `${k ? "L" : "M"}${x(c.at_sec)},${y(c.offset_deg!)}`).join(" ");
  return (
    <figure style={{ margin: 0 }}>
      <svg className="drift-chart" viewBox={`0 0 ${W} ${H}`} role="img" aria-label={t.alt(checks.length)}>
        <line className="axis" x1={PAD.l} y1={H - PAD.b} x2={W - PAD.r} y2={H - PAD.b} />
        <line className="axis" x1={PAD.l} y1={PAD.t} x2={PAD.l} y2={H - PAD.b} />
        <text x={PAD.l - 6} y={y(0) + 4} textAnchor="end">0</text>
        <text x={PAD.l - 6} y={PAD.t + 8} textAnchor="end">{maxDeg.toFixed(1)}</text>
        <text x={(W + PAD.l) / 2} y={H - 4} textAnchor="middle">{t.axisSec}</text>
        {lines.map((l) => (
          <g key={l.key} data-testid={`drift-${l.key}`}>
            <line className={l.key} x1={PAD.l} x2={W - PAD.r} y1={y(l.deg)} y2={y(l.deg)} />
            <text x={W - PAD.r} y={y(l.deg) - 3} textAnchor="end">{`${l.label} ${l.deg}°`}</text>
          </g>
        ))}
        {path ? <path className="line" d={path} /> : null}
        {checks.map((c) =>
          c.offset_deg == null ? (
            <text key={c.at_sec} x={x(c.at_sec)} y={H - PAD.b - 4} textAnchor="middle">{`× ${t.missed}`}</text>
          ) : (
            <circle key={c.at_sec} cx={x(c.at_sec)} cy={y(c.offset_deg)} r={4} fill="currentColor">
              <title>{`${c.at_sec.toFixed(1)} s: ${c.offset_deg.toFixed(2)}°`}</title>
            </circle>
          ),
        )}
      </svg>
      <figcaption className="muted">
        <ul className="legend">
          {checks.map((c) => (
            <li key={c.at_sec}>
              {c.offset_deg == null
                ? `${c.at_sec.toFixed(1)} s: ${t.missed}`
                : `${c.at_sec.toFixed(1)} s: ${en.detail.tracking.deg(c.offset_deg)}`}
            </li>
          ))}
        </ul>
      </figcaption>
    </figure>
  );
}
