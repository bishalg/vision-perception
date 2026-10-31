/** Synthetic schema 1.7 tracking-quality records for the mock client (WP-G.12). Deterministic, no real data. */
import type { SessionTrackingQuality, TrackingVerdict } from "../types";

/** Verdict cycle for 1.7 mock sessions: mostly good, with one of each worse verdict per cycle. */
export const MOCK_TRACKING_CYCLE: TrackingVerdict[] = ["good", "good", "fair", "poor", "unusable"];

const BY_VERDICT: Record<TrackingVerdict, { acc: number; validation: number | null; drift: (number | null)[]; loss: number }> = {
  good: { acc: 1.4, validation: 1.2, drift: [0.4, 0.6, 0.5], loss: 0.8 },
  fair: { acc: 2.4, validation: 2.1, drift: [0.9, 1.7, 1.9], loss: 2.5 },
  poor: { acc: 3.9, validation: 3.8, drift: [1.6, 2.4, 3.4], loss: 6 },
  unusable: { acc: 6.2, validation: null, drift: [null, 3.8], loss: 14 },
};

export function mockTrackingQuality(verdict: TrackingVerdict, durationSec: number): SessionTrackingQuality {
  const v = BY_VERDICT[verdict];
  const step = durationSec / (v.drift.length + 1);
  const failed = verdict === "unusable";
  return {
    pipeline_version: "asd-gaze/1.0.0",
    params_version: "1.0.0",
    device: {
      model_identifier: "iPad13,18",
      geometry_source: "table",
      orientation: "landscapeLeft",
      screen_width_mm: 238.25,
      screen_height_mm: 165.64,
      sample_rate_hz: 60,
    },
    calibration: {
      layout_points: 5,
      usable_points: failed ? 2 : 5,
      missing_points: failed ? 3 : 0,
      quality: failed ? "failed" : verdict === "poor" ? "poor" : verdict === "fair" ? "fair" : "good",
      attempts: failed ? 3 : 1,
      model: failed ? null : "screenAffine",
      residual_deg: failed ? null : 1.1,
      loo_cv_deg: failed ? null : v.acc,
      condition_number: failed ? null : 42.5,
      age_sec_at_start: 6.5,
      pose: failed ? null : { distance_cm: 38, yaw_deg: 3.5, pitch_deg: -4.25 },
    },
    validation: failed
      ? null
      : {
          complete: true,
          data_loss: verdict === "good" ? 0.04 : verdict === "fair" ? 0.1 : 0.25,
          targets: [],
          accuracy_deg: v.validation,
          precision_rms_deg: 0.22,
          precision_rms_raw_deg: 0.48,
        },
    drift_checks: v.drift.map((offset, k) => ({
      at_sec: Math.round(step * (k + 1) * 10) / 10,
      x_norm: 0.5,
      y_norm: 0.5,
      missed: offset === null,
      offset_deg: offset,
      drift_deg: offset,
    })),
    max_drift_deg: failed ? 3.8 : v.drift[v.drift.length - 1],
    recalibrations: verdict === "poor" ? [{ at_sec: 40, reason: "drift", new_quality: "fair" }] : [],
    conditions: {
      distance_cm_median: failed ? null : 39.5,
      distance_cm_iqr: failed ? null : 4.25,
      head_speed_deg_per_sec_p90: 18.5,
      ambient_intensity_median: 870,
      light_asymmetry: 0.12,
      glasses: verdict === "fair",
    },
    loss: {
      track_loss_sec: v.loss,
      blink_sec: 0.6,
      offscreen_sec: 0.4,
      artifact_sec: 0.05,
      gap_filled_sec: 0.3,
      longest_gap_sec: 0.45,
    },
    confidence: { mean_sample: 0.82, low_confidence_sample_ratio: 0.07, mean_fixation: 0.86 },
    estimated_accuracy_deg: v.acc,
    verdict,
  };
}
