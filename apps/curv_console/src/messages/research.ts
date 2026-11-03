/**
 * User-visible strings of the researcher toolkit (ADR-0012; RUI-*), English and Nepali (ADR-0013 principle 2). Both
 * locales have the same shape (`ResearchMessages`); a `ne` text marked `// ne: draft` is a machine draft until WP-L.3
 * reviews it. WP-X.5 / WP-X.7 add the page strings here.
 *
 * `params` holds one label and help text per parameter-registry row (PRM-5, key `param.<id>.label|help` on the Rust
 * side). The blocks between the `<registry:...>` markers are maintained by `scripts/gen-registry-messages.mjs` from
 * `docs/spec/schemas/asd/param-registry-v1.json`: it adds new ids with "TODO" and keeps written values. WP-X.5 fills
 * the values (its `messages.test.ts` fails on a "TODO").
 */
import type { REPROCESS_LEVELS, RunStatus } from "@/lib/api/research";

export interface ParamText {
  label: string;
  help: string;
}

export interface ResearchMessages {
  nav: string;
  title: string;
  /** PRV-6: on every research page and export README; the English sentence is always present. */
  disclaimer: string;
  notAvailable: string;
  /** A suppressed cell (PRV-1): never shown as a number. */
  suppressedCell: (k: number) => string;
  runStatus: Record<RunStatus, string>;
  reprocess: Record<(typeof REPROCESS_LEVELS)[number], string>;
  level: { auto: string; score: string; frames: string };
  ownerGated: string;
  params: Record<string, ParamText>;
}

const en: ResearchMessages = {
  nav: "Research",
  title: "Research",
  disclaimer: "This is not a diagnostic tool.",
  notAvailable: "This part of the research area is not available yet.",
  suppressedCell: (k) => `n < ${k}`,
  runStatus: {
    queued: "Queued",
    running: "Running",
    succeeded: "Finished",
    failed: "Failed",
    cancelled: "Cancelled",
  },
  reprocess: {
    score: "score",
    metrics: "metrics",
    frames: "frames",
    capture_only: "capture only",
  },
  level: { auto: "Automatic", score: "Score level", frames: "Frame level" },
  ownerGated: "Fenced for clinical use; analysis only",
  params: {
    // <registry:en:begin>
    "flag.greenFloor": { label: "TODO", help: "TODO" },
    "flag.yellowFloor": { label: "TODO", help: "TODO" },
    "flag.lowConfidenceShare": { label: "TODO", help: "TODO" },
    "flag.atypicalShareForRed": { label: "TODO", help: "TODO" },
    "lowConfidenceValidRatio": { label: "TODO", help: "TODO" },
    "qualityGate.minDistanceCm": { label: "TODO", help: "TODO" },
    "qualityGate.maxDistanceCm": { label: "TODO", help: "TODO" },
    "qualityGate.maxYawRad": { label: "TODO", help: "TODO" },
    "qualityGate.maxPitchRad": { label: "TODO", help: "TODO" },
    "qualityGate.minConfidence": { label: "TODO", help: "TODO" },
    "qualityGate.blinkThreshold": { label: "TODO", help: "TODO" },
    "trackingQuality.provisional": { label: "TODO", help: "TODO" },
    "trackingQuality.calibration.settleMs": { label: "TODO", help: "TODO" },
    "trackingQuality.calibration.collectMs": { label: "TODO", help: "TODO" },
    "trackingQuality.calibration.maxTargetWaitMs": { label: "TODO", help: "TODO" },
    "trackingQuality.calibration.minSamplesPerTarget": { label: "TODO", help: "TODO" },
    "trackingQuality.calibration.maxRetriesPerTarget": { label: "TODO", help: "TODO" },
    "trackingQuality.calibration.madRejectK": { label: "TODO", help: "TODO" },
    "trackingQuality.calibration.defaultModel": { label: "TODO", help: "TODO" },
    "trackingQuality.calibration.autoSelect": { label: "TODO", help: "TODO" },
    "trackingQuality.calibration.minImprovementDeg": { label: "TODO", help: "TODO" },
    "trackingQuality.calibration.eligibilityMarginPoints": { label: "TODO", help: "TODO" },
    "trackingQuality.calibration.bands.goodMaxDeg": { label: "TODO", help: "TODO" },
    "trackingQuality.calibration.bands.fairMaxDeg": { label: "TODO", help: "TODO" },
    "trackingQuality.calibration.bands.poorMaxDeg": { label: "TODO", help: "TODO" },
    "trackingQuality.calibration.minUsablePoints": { label: "TODO", help: "TODO" },
    "trackingQuality.calibration.maxConditionNumber": { label: "TODO", help: "TODO" },
    "trackingQuality.calibration.layout5": { label: "TODO", help: "TODO" },
    "trackingQuality.calibration.layout9": { label: "TODO", help: "TODO" },
    "trackingQuality.recalibration.poseShiftDistancePct": { label: "TODO", help: "TODO" },
    "trackingQuality.recalibration.poseShiftAngleDeg": { label: "TODO", help: "TODO" },
    "trackingQuality.recalibration.poseWindowSec": { label: "TODO", help: "TODO" },
    "trackingQuality.recalibration.maxCalibrationAgeSec": { label: "TODO", help: "TODO" },
    "trackingQuality.recalibration.maxRecalibrationsPerSession": { label: "TODO", help: "TODO" },
    "trackingQuality.recalibration.maxRecalibrationsInBattery": { label: "TODO", help: "TODO" },
    "trackingQuality.validation.layout": { label: "TODO", help: "TODO" },
    "trackingQuality.validation.settleMs": { label: "TODO", help: "TODO" },
    "trackingQuality.validation.collectMs": { label: "TODO", help: "TODO" },
    "trackingQuality.validation.minSamplesPerTarget": { label: "TODO", help: "TODO" },
    "trackingQuality.validation.minTargetsHit": { label: "TODO", help: "TODO" },
    "trackingQuality.validation.warnAccuracyDeg": { label: "TODO", help: "TODO" },
    "trackingQuality.validation.maxAccuracyDeg": { label: "TODO", help: "TODO" },
    "trackingQuality.validation.benchLayout13": { label: "TODO", help: "TODO" },
    "trackingQuality.drift.checkLayout": { label: "TODO", help: "TODO" },
    "trackingQuality.drift.settleMs": { label: "TODO", help: "TODO" },
    "trackingQuality.drift.collectMs": { label: "TODO", help: "TODO" },
    "trackingQuality.drift.minSamples": { label: "TODO", help: "TODO" },
    "trackingQuality.drift.warnDeg": { label: "TODO", help: "TODO" },
    "trackingQuality.drift.warnConsecutive": { label: "TODO", help: "TODO" },
    "trackingQuality.drift.hardDeg": { label: "TODO", help: "TODO" },
    "trackingQuality.trackState.blinkRecoveryMs": { label: "TODO", help: "TODO" },
    "trackingQuality.trackState.maxEyeSpeedDegPerSec": { label: "TODO", help: "TODO" },
    "trackingQuality.trackState.offscreenMarginNorm": { label: "TODO", help: "TODO" },
    "trackingQuality.confidence.eyeOpennessComfort": { label: "TODO", help: "TODO" },
    "trackingQuality.confidence.poseComfortFraction": { label: "TODO", help: "TODO" },
    "trackingQuality.confidence.distanceComfortMinCm": { label: "TODO", help: "TODO" },
    "trackingQuality.confidence.distanceComfortMaxCm": { label: "TODO", help: "TODO" },
    "trackingQuality.confidence.headSpeedComfortDegPerSec": { label: "TODO", help: "TODO" },
    "trackingQuality.confidence.maxHeadSpeedDegPerSec": { label: "TODO", help: "TODO" },
    "trackingQuality.confidence.poseShiftComfortPct": { label: "TODO", help: "TODO" },
    "trackingQuality.gapFill.maxMs": { label: "TODO", help: "TODO" },
    "trackingQuality.gapFill.maxDeg": { label: "TODO", help: "TODO" },
    "trackingQuality.gapFill.lossIntervalCapMs": { label: "TODO", help: "TODO" },
    "trackingQuality.filter.minCutoffHz": { label: "TODO", help: "TODO" },
    "trackingQuality.filter.beta": { label: "TODO", help: "TODO" },
    "trackingQuality.filter.dCutoffHz": { label: "TODO", help: "TODO" },
    "trackingQuality.verdict.accuracyBands.goodMaxDeg": { label: "TODO", help: "TODO" },
    "trackingQuality.verdict.accuracyBands.fairMaxDeg": { label: "TODO", help: "TODO" },
    "trackingQuality.verdict.accuracyBands.poorMaxDeg": { label: "TODO", help: "TODO" },
    "trackingQuality.verdict.unusableBelowValidRatio": { label: "TODO", help: "TODO" },
    "trackingQuality.verdict.poorAboveDataLoss": { label: "TODO", help: "TODO" },
    "trackingQuality.verdict.fairAboveDataLoss": { label: "TODO", help: "TODO" },
    "trackingQuality.scoring.maxAccuracyDegForScoring": { label: "TODO", help: "TODO" },
    "trackingQuality.scoring.aoiResolutionFactor": { label: "TODO", help: "TODO" },
    "trackingQuality.scoring.ivtVelocityDegPerSec": { label: "TODO", help: "TODO" },
    "metrics.ivtVelocityUnitsPerSec": { label: "TODO", help: "TODO" },
    "metrics.ivtVelocityDegPerSec": { label: "TODO", help: "TODO" },
    "metrics.ivtUnit": { label: "TODO", help: "TODO" },
    "metrics.minFixationDurationSec": { label: "TODO", help: "TODO" },
    "metrics.fallbackFrameIntervalSec": { label: "TODO", help: "TODO" },
    "metrics.smileReactivityWindowSec": { label: "TODO", help: "TODO" },
    "metrics.flatAffectGain": { label: "TODO", help: "TODO" },
    "aoi.marginNorm": { label: "TODO", help: "TODO" },
    "aoi.dwellCapMs": { label: "TODO", help: "TODO" },
    "ageBands": { label: "TODO", help: "TODO" },
    "norms.minN": { label: "TODO", help: "TODO" },
    "norms.includeImputed": { label: "TODO", help: "TODO" },
    "review.lowQualityValidRatio": { label: "TODO", help: "TODO" },
    // <registry:en:end>
  },
};

const ne: ResearchMessages = {
  nav: "अनुसन्धान", // ne: draft
  title: "अनुसन्धान", // ne: draft
  // ne: draft (the English sentence stays, PRV-6)
  disclaimer: "यो निदान गर्ने उपकरण होइन। This is not a diagnostic tool.",
  notAvailable: "अनुसन्धानको यो भाग अझै उपलब्ध छैन।", // ne: draft
  suppressedCell: (k) => `n < ${k}`,
  runStatus: {
    queued: "पालोमा", // ne: draft
    running: "चलिरहेको", // ne: draft
    succeeded: "सम्पन्न", // ne: draft
    failed: "असफल", // ne: draft
    cancelled: "रद्द गरिएको", // ne: draft
  },
  reprocess: {
    score: "स्कोर", // ne: draft
    metrics: "मेट्रिक्स", // ne: draft
    frames: "फ्रेम", // ne: draft
    capture_only: "क्याप्चर मात्र", // ne: draft
  },
  level: { auto: "स्वचालित", score: "स्कोर तह", frames: "फ्रेम तह" }, // ne: draft
  ownerGated: "क्लिनिकल प्रयोगका लागि बन्द; विश्लेषणमा मात्र", // ne: draft
  // ne: draft (registry labels: WP-X.5 writes them, WP-L.3 reviews)
  params: {
    // <registry:ne:begin>
    "flag.greenFloor": { label: "TODO", help: "TODO" },
    "flag.yellowFloor": { label: "TODO", help: "TODO" },
    "flag.lowConfidenceShare": { label: "TODO", help: "TODO" },
    "flag.atypicalShareForRed": { label: "TODO", help: "TODO" },
    "lowConfidenceValidRatio": { label: "TODO", help: "TODO" },
    "qualityGate.minDistanceCm": { label: "TODO", help: "TODO" },
    "qualityGate.maxDistanceCm": { label: "TODO", help: "TODO" },
    "qualityGate.maxYawRad": { label: "TODO", help: "TODO" },
    "qualityGate.maxPitchRad": { label: "TODO", help: "TODO" },
    "qualityGate.minConfidence": { label: "TODO", help: "TODO" },
    "qualityGate.blinkThreshold": { label: "TODO", help: "TODO" },
    "trackingQuality.provisional": { label: "TODO", help: "TODO" },
    "trackingQuality.calibration.settleMs": { label: "TODO", help: "TODO" },
    "trackingQuality.calibration.collectMs": { label: "TODO", help: "TODO" },
    "trackingQuality.calibration.maxTargetWaitMs": { label: "TODO", help: "TODO" },
    "trackingQuality.calibration.minSamplesPerTarget": { label: "TODO", help: "TODO" },
    "trackingQuality.calibration.maxRetriesPerTarget": { label: "TODO", help: "TODO" },
    "trackingQuality.calibration.madRejectK": { label: "TODO", help: "TODO" },
    "trackingQuality.calibration.defaultModel": { label: "TODO", help: "TODO" },
    "trackingQuality.calibration.autoSelect": { label: "TODO", help: "TODO" },
    "trackingQuality.calibration.minImprovementDeg": { label: "TODO", help: "TODO" },
    "trackingQuality.calibration.eligibilityMarginPoints": { label: "TODO", help: "TODO" },
    "trackingQuality.calibration.bands.goodMaxDeg": { label: "TODO", help: "TODO" },
    "trackingQuality.calibration.bands.fairMaxDeg": { label: "TODO", help: "TODO" },
    "trackingQuality.calibration.bands.poorMaxDeg": { label: "TODO", help: "TODO" },
    "trackingQuality.calibration.minUsablePoints": { label: "TODO", help: "TODO" },
    "trackingQuality.calibration.maxConditionNumber": { label: "TODO", help: "TODO" },
    "trackingQuality.calibration.layout5": { label: "TODO", help: "TODO" },
    "trackingQuality.calibration.layout9": { label: "TODO", help: "TODO" },
    "trackingQuality.recalibration.poseShiftDistancePct": { label: "TODO", help: "TODO" },
    "trackingQuality.recalibration.poseShiftAngleDeg": { label: "TODO", help: "TODO" },
    "trackingQuality.recalibration.poseWindowSec": { label: "TODO", help: "TODO" },
    "trackingQuality.recalibration.maxCalibrationAgeSec": { label: "TODO", help: "TODO" },
    "trackingQuality.recalibration.maxRecalibrationsPerSession": { label: "TODO", help: "TODO" },
    "trackingQuality.recalibration.maxRecalibrationsInBattery": { label: "TODO", help: "TODO" },
    "trackingQuality.validation.layout": { label: "TODO", help: "TODO" },
    "trackingQuality.validation.settleMs": { label: "TODO", help: "TODO" },
    "trackingQuality.validation.collectMs": { label: "TODO", help: "TODO" },
    "trackingQuality.validation.minSamplesPerTarget": { label: "TODO", help: "TODO" },
    "trackingQuality.validation.minTargetsHit": { label: "TODO", help: "TODO" },
    "trackingQuality.validation.warnAccuracyDeg": { label: "TODO", help: "TODO" },
    "trackingQuality.validation.maxAccuracyDeg": { label: "TODO", help: "TODO" },
    "trackingQuality.validation.benchLayout13": { label: "TODO", help: "TODO" },
    "trackingQuality.drift.checkLayout": { label: "TODO", help: "TODO" },
    "trackingQuality.drift.settleMs": { label: "TODO", help: "TODO" },
    "trackingQuality.drift.collectMs": { label: "TODO", help: "TODO" },
    "trackingQuality.drift.minSamples": { label: "TODO", help: "TODO" },
    "trackingQuality.drift.warnDeg": { label: "TODO", help: "TODO" },
    "trackingQuality.drift.warnConsecutive": { label: "TODO", help: "TODO" },
    "trackingQuality.drift.hardDeg": { label: "TODO", help: "TODO" },
    "trackingQuality.trackState.blinkRecoveryMs": { label: "TODO", help: "TODO" },
    "trackingQuality.trackState.maxEyeSpeedDegPerSec": { label: "TODO", help: "TODO" },
    "trackingQuality.trackState.offscreenMarginNorm": { label: "TODO", help: "TODO" },
    "trackingQuality.confidence.eyeOpennessComfort": { label: "TODO", help: "TODO" },
    "trackingQuality.confidence.poseComfortFraction": { label: "TODO", help: "TODO" },
    "trackingQuality.confidence.distanceComfortMinCm": { label: "TODO", help: "TODO" },
    "trackingQuality.confidence.distanceComfortMaxCm": { label: "TODO", help: "TODO" },
    "trackingQuality.confidence.headSpeedComfortDegPerSec": { label: "TODO", help: "TODO" },
    "trackingQuality.confidence.maxHeadSpeedDegPerSec": { label: "TODO", help: "TODO" },
    "trackingQuality.confidence.poseShiftComfortPct": { label: "TODO", help: "TODO" },
    "trackingQuality.gapFill.maxMs": { label: "TODO", help: "TODO" },
    "trackingQuality.gapFill.maxDeg": { label: "TODO", help: "TODO" },
    "trackingQuality.gapFill.lossIntervalCapMs": { label: "TODO", help: "TODO" },
    "trackingQuality.filter.minCutoffHz": { label: "TODO", help: "TODO" },
    "trackingQuality.filter.beta": { label: "TODO", help: "TODO" },
    "trackingQuality.filter.dCutoffHz": { label: "TODO", help: "TODO" },
    "trackingQuality.verdict.accuracyBands.goodMaxDeg": { label: "TODO", help: "TODO" },
    "trackingQuality.verdict.accuracyBands.fairMaxDeg": { label: "TODO", help: "TODO" },
    "trackingQuality.verdict.accuracyBands.poorMaxDeg": { label: "TODO", help: "TODO" },
    "trackingQuality.verdict.unusableBelowValidRatio": { label: "TODO", help: "TODO" },
    "trackingQuality.verdict.poorAboveDataLoss": { label: "TODO", help: "TODO" },
    "trackingQuality.verdict.fairAboveDataLoss": { label: "TODO", help: "TODO" },
    "trackingQuality.scoring.maxAccuracyDegForScoring": { label: "TODO", help: "TODO" },
    "trackingQuality.scoring.aoiResolutionFactor": { label: "TODO", help: "TODO" },
    "trackingQuality.scoring.ivtVelocityDegPerSec": { label: "TODO", help: "TODO" },
    "metrics.ivtVelocityUnitsPerSec": { label: "TODO", help: "TODO" },
    "metrics.ivtVelocityDegPerSec": { label: "TODO", help: "TODO" },
    "metrics.ivtUnit": { label: "TODO", help: "TODO" },
    "metrics.minFixationDurationSec": { label: "TODO", help: "TODO" },
    "metrics.fallbackFrameIntervalSec": { label: "TODO", help: "TODO" },
    "metrics.smileReactivityWindowSec": { label: "TODO", help: "TODO" },
    "metrics.flatAffectGain": { label: "TODO", help: "TODO" },
    "aoi.marginNorm": { label: "TODO", help: "TODO" },
    "aoi.dwellCapMs": { label: "TODO", help: "TODO" },
    "ageBands": { label: "TODO", help: "TODO" },
    "norms.minN": { label: "TODO", help: "TODO" },
    "norms.includeImputed": { label: "TODO", help: "TODO" },
    "review.lowQualityValidRatio": { label: "TODO", help: "TODO" },
    // <registry:ne:end>
  },
};

export const researchMessages = { en, ne } as const;
export type ResearchLocale = keyof typeof researchMessages;

/** The console's current catalogue (English until the locale switch of WP-L.1). */
export const researchText = en;
