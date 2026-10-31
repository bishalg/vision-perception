import { en } from "@/messages/en";
import type { SessionTrackingQuality } from "@/lib/api";

const t = en.detail.tracking.loss;

/**
 * Data loss by track state (disjoint states, seconds from the API). "Tracked" is the remainder of the
 * session duration; every other segment is plotted as sent.
 */
export function LossBar({ loss, durationSec }: { loss: SessionTrackingQuality["loss"]; durationSec: number }) {
  const parts = [
    { key: "blink", label: t.blink, sec: loss.blink_sec },
    { key: "lost", label: t.lost, sec: loss.track_loss_sec },
    { key: "offscreen", label: t.offscreen, sec: loss.offscreen_sec },
    { key: "artifact", label: t.artifact, sec: loss.artifact_sec },
  ];
  const lossTotal = parts.reduce((sum, p) => sum + p.sec, 0);
  const total = Math.max(durationSec, lossTotal);
  const segments = [{ key: "tracked", label: t.tracked, sec: total - lossTotal }, ...parts];
  return (
    <figure style={{ margin: 0 }}>
      <div className="lossbar" role="img" aria-label={segments.map((s) => t.seconds(s.label, s.sec)).join(", ")}>
        {segments.map((s) =>
          s.sec > 0 ? (
            <span key={s.key} className={`seg-${s.key}`} style={{ flexGrow: s.sec }} title={t.seconds(s.label, s.sec)} />
          ) : null,
        )}
      </div>
      <ul className="legend">
        {segments.map((s) => (
          <li key={s.key}>{t.seconds(s.label, s.sec)}</li>
        ))}
      </ul>
    </figure>
  );
}
