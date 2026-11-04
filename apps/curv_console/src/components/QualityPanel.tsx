"use client";

import type { SessionDetail } from "@/lib/api";
import { formatNumber, formatPercent } from "@/lib/format";
import { useText } from "@/lib/i18n/provider";
import { enumLabel } from "@/messages/domain";
import { notScoredLabel } from "./ScoringStatus";
import { TrackingQuality } from "./TrackingQuality";

export function QualityPanel({
  session,
  warnDeg,
  hardDeg,
}: {
  session: SessionDetail;
  /** Drift thresholds of the effective scoring profile; absent draws no lines. */
  warnDeg?: number | null;
  hardDeg?: number | null;
}) {
  const t = useText();
  const q = t.app.detail.quality_panel;
  const rows: [string, string][] = [
    [q.validFrames, formatPercent(session.valid_frame_ratio, t.locale)],
    [q.distance, session.calibrated_distance_cm == null ? q.notMeasured : formatNumber(session.calibrated_distance_cm, 0, t.locale)],
    [q.captureSource, session.capture_source ? enumLabel(t.locale, "capture_source", session.capture_source) : "-"],
    [q.socialPreference, notScoredLabel(session, t.app) ?? formatPercent(session.social_preference, t.locale)],
    [q.eyeDwell, formatPercent(session.eye_dwell, t.locale)],
    [q.mouthDwell, formatPercent(session.mouth_dwell, t.locale)],
    [q.firstEyeLatency, formatNumber(session.first_eye_latency_ms, 0, t.locale)],
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
      <TrackingQuality session={session} warnDeg={warnDeg} hardDeg={hardDeg} />
    </>
  );
}
