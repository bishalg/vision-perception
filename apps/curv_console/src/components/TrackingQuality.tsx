import type { ReactNode } from "react";
import { en } from "@/messages/en";
import type { SessionDetail, TrackingVerdict } from "@/lib/api";
import { formatNumber, formatPercent } from "@/lib/format";
import { DriftChart } from "./DriftChart";
import { LossBar } from "./LossBar";

const t = en.detail.tracking;

/** Text label always present: colour is never the only signal. */
export function TrackingBadge({ verdict }: { verdict: TrackingVerdict | null | undefined }) {
  if (!verdict) return <span className="muted">{t.notMeasured}</span>;
  return <span className={`badge badge-trk-${verdict}`}>{t.verdicts[verdict]}</span>;
}

function deg(v: number | null | undefined): string {
  return v == null ? t.notMeasured : t.deg(v);
}
function num(v: number | null | undefined, digits = 1): string {
  return v == null ? t.notMeasured : formatNumber(v, digits);
}

function Fact({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div>
      <dt>{label}</dt>
      <dd>{children}</dd>
    </div>
  );
}

export function TrackingQuality({
  session,
  warnDeg,
  hardDeg,
}: {
  session: SessionDetail;
  warnDeg?: number | null;
  hardDeg?: number | null;
}) {
  const q = session.tracking_quality;
  return (
    <section className="tracking" aria-labelledby="tracking-h">
      <h2 id="tracking-h">{t.title}</h2>
      {!q ? (
        <p className="muted">{t.notRecorded}</p>
      ) : (
        <>
          <dl className="facts">
            <Fact label={t.verdict}><TrackingBadge verdict={q.verdict} /></Fact>
            <Fact label={t.estimatedAccuracy}>{deg(q.estimated_accuracy_deg)}</Fact>
            <Fact label={t.validationAccuracy}>{deg(q.validation?.accuracy_deg)}</Fact>
            <Fact label={t.validationPrecision}>{deg(q.validation?.precision_rms_deg)}</Fact>
            <Fact label={t.validationDataLoss}>{q.validation ? formatPercent(q.validation.data_loss) : t.notMeasured}</Fact>
            <Fact label={t.calibrationQuality}>{q.calibration.quality}</Fact>
            <Fact label={t.calibrationModel}>{q.calibration.model ?? t.notMeasured}</Fact>
            <Fact label={t.looCv}>{deg(q.calibration.loo_cv_deg)}</Fact>
            <Fact label={t.device}>{q.device.model_identifier}</Fact>
            <Fact label={t.geometry}>{q.device.geometry_source}</Fact>
            <Fact label={t.pipeline}>{q.pipeline_version}</Fact>
          </dl>
          <h3>{t.loss.title}</h3>
          <LossBar loss={q.loss} durationSec={session.duration_sec} />
          <h3>{t.drift.title}</h3>
          <DriftChart checks={q.drift_checks} warnDeg={warnDeg} hardDeg={hardDeg} />
          <h3>{t.conditions.title}</h3>
          <dl className="facts">
            <Fact label={t.conditions.distanceMedian}>{num(q.conditions.distance_cm_median, 0)}</Fact>
            <Fact label={t.conditions.distanceIqr}>{num(q.conditions.distance_cm_iqr)}</Fact>
            <Fact label={t.conditions.headSpeed}>{num(q.conditions.head_speed_deg_per_sec_p90)}</Fact>
            <Fact label={t.conditions.ambient}>{num(q.conditions.ambient_intensity_median, 0)}</Fact>
            <Fact label={t.conditions.asymmetry}>{num(q.conditions.light_asymmetry, 2)}</Fact>
            <Fact label={t.conditions.glasses}>
              {q.conditions.glasses == null ? t.notMeasured : q.conditions.glasses ? t.conditions.yes : t.conditions.no}
            </Fact>
          </dl>
        </>
      )}
    </section>
  );
}
