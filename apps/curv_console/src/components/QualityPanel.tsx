import { en } from "@/messages/en";
import type { SessionDetail } from "@/lib/api";
import { formatNumber, formatPercent } from "@/lib/format";
import { TrackingQuality } from "./TrackingQuality";

const q = en.detail.quality_panel;

export function QualityPanel({ session }: { session: SessionDetail }) {
  const rows: [string, string][] = [
    [q.validFrames, formatPercent(session.valid_frame_ratio)],
    [q.distance, session.calibrated_distance_cm == null ? q.notMeasured : formatNumber(session.calibrated_distance_cm, 0)],
    [q.captureSource, session.capture_source ?? "-"],
    [q.socialPreference, formatPercent(session.social_preference)],
    [q.eyeDwell, formatPercent(session.eye_dwell)],
    [q.mouthDwell, formatPercent(session.mouth_dwell)],
    [q.firstEyeLatency, formatNumber(session.first_eye_latency_ms, 0)],
  ];
  return (
    <>
      <dl className="facts">
        {rows.map(([label, value]) => (
          <div key={label}>
            <dt>{label}</dt>
            <dd>{value}</dd>
          </div>
        ))}
      </dl>
      <TrackingQuality session={session} />
    </>
  );
}
