/**
 * Mock-mode baselines and scoring profiles (WP-C.5). Synthetic: the baseline set covers the mock sessions' clips and
 * metric rows (`../mock.ts` CLIPS and METRICS) so the sandbox re-score has something to score in mock mode; the
 * values are illustrative, not clinical norms.
 */
import type { NormativeBaselineSet, NormativeExpectation, ScoringProfile } from "../config";

/** The mock sessions' clip ids (same list as `../mock.ts`). */
export const MOCK_SESSION_CLIPS = [
  "stim_01_social_geo_10s", "stim_02_gaze_cue_8s", "stim_03_bio_motion_10s", "stim_04_face_eye_8s",
  "stim_05_face_mouth_8s", "stim_06_joint_attn_10s", "stim_07_orienting_6s", "stim_08_affect_10s",
] as const;

export const MOCK_BASELINES_VERSIONS = {
  bundled: "bundled-1.0.0",
  published: "2026.10.1",
  draft: "2026.11.0-rc1",
} as const;

export const MOCK_SCORING_VERSIONS = {
  bundled: "bundled-1.0.0",
  published: "2026.10.1",
  candidate: "2026.11.0-rc1",
} as const;

const BRACKETS = [
  { id: "12-17", minMonths: 12, maxMonths: 17 },
  { id: "18-23", minMonths: 18, maxMonths: 23 },
  { id: "24-35", minMonths: 24, maxMonths: 35 },
  { id: "36-47", minMonths: 36, maxMonths: 47 },
  { id: "48-60", minMonths: 48, maxMonths: 60 },
];

function expectations(strict: boolean): NormativeExpectation[] {
  const s = strict ? 0.05 : 0;
  return [
    {
      biomarker: "social_dwell_ratio",
      direction: "higherBetter",
      targetAOI: "social",
      byAge: {
        default: { expected: 0.45 + s, acceptableLow: 0.3 + s },
        "12-17": { expected: 0.4 + s, acceptableLow: 0.25 + s },
      },
      citation: "Synthetic mock value (social preference split screen).",
      source: "literature",
    },
    {
      biomarker: "eye_dwell_ratio",
      direction: "higherBetter",
      targetAOI: "eyes",
      byAge: { default: { expected: 0.35 + s, acceptableLow: 0.2 + s } },
      citation: "Synthetic mock value (eye-region dwell).",
      source: strict ? "own:mock-norm-run-7, n=48" : "literature",
    },
    {
      biomarker: "mouth_dwell_ratio",
      direction: "band",
      targetAOI: "mouth",
      byAge: { default: { expected: 0.3, acceptableLow: 0.15, acceptableHigh: 0.45 } },
      citation: "Synthetic mock value (mouth dwell band).",
      source: "interpolated",
    },
    {
      biomarker: "orienting_latency_ms",
      direction: "lowerBetter",
      targetAOI: null,
      byAge: { default: { expected: 0.4, acceptableHigh: 0.6 - s } },
      citation: "Synthetic mock value (orienting latency, scaled).",
      source: "literature",
    },
  ];
}

export function mockBaselineSet(strict = false): NormativeBaselineSet {
  const videos: NormativeBaselineSet["videos"] = {};
  MOCK_SESSION_CLIPS.forEach((id, i) => {
    const seconds = Number(/_(\d+)s$/.exec(id)?.[1] ?? 8);
    videos[id] = {
      title: id.replace(/^stim_\d+_/, "").replace(/_\d+s$/, "").replace(/_/g, " "),
      category: i % 2 ? "Social" : "Attention",
      weight: 0.125,
      segments: [{ id: "full", description: "Whole clip", startSec: 0, endSec: seconds, expectations: expectations(strict) }],
    };
  });
  return {
    schemaVersion: "1.0.0",
    status: strict ? "candidate-norms" : "provisional-literature-seeded",
    provenance: strict ? "Mock norm run 7 merged over the literature seed." : "Mock literature seed.",
    ageBrackets: BRACKETS,
    videos,
  };
}

/** Same values as the Rust `ScoringProfile::default()` (no `trackingQuality`: it decodes to the core defaults). */
export function mockScoringProfile(strict = false): ScoringProfile {
  return {
    schemaVersion: "1.1.0",
    flag: {
      greenFloor: strict ? 0.75 : 0.65,
      yellowFloor: strict ? 0.45 : 0.35,
      lowConfidenceShare: 0.5,
      atypicalShareForRed: strict ? 0.3 : 0.4,
    },
    lowConfidenceValidRatio: 0.6,
    qualityGate: {
      minDistanceCm: 35,
      maxDistanceCm: 65,
      maxYawRad: 0.61,
      maxPitchRad: 0.52,
      minConfidence: 0.5,
      blinkThreshold: 0.45,
    },
  };
}
