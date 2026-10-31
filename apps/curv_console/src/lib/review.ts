/**
 * Review-queue triage vocabulary (spec 03 §4.2): what each console action sends as `PATCH /v1/review/{id}`.
 * This is UI wiring only; the backend validates the request, writes `labels` + `audit_log`, and the
 * norm-aggregation job reads `exclude_from_norms` (label `true`, or a `done` decision with `exclude_from_norms: true`).
 */
import type { UpdateReviewRequest } from "@/lib/api";

/** In keyboard order: keys 1-4. */
export const TRIAGE_ACTIONS = ["confirm", "capture_issue", "exclude_from_norms", "dismiss"] as const;
export type TriageAction = (typeof TRIAGE_ACTIONS)[number];

/** Labels a reviewer can attach with "Flag capture issue". */
export const CAPTURE_ISSUE_LABELS = ["child_distracted", "adult_in_frame", "calibration_drift", "poor_lighting", "other"] as const;
export type CaptureIssueLabel = (typeof CAPTURE_ISSUE_LABELS)[number];

export function isTriageAction(value: unknown): value is TriageAction {
  return typeof value === "string" && (TRIAGE_ACTIONS as readonly string[]).includes(value);
}

export function isCaptureIssueLabel(value: unknown): value is CaptureIssueLabel {
  return typeof value === "string" && (CAPTURE_ISSUE_LABELS as readonly string[]).includes(value);
}

/** "1".."4" -> action; anything else -> null. */
export function triageForKey(key: string): TriageAction | null {
  const i = ["1", "2", "3", "4"].indexOf(key);
  return i < 0 ? null : TRIAGE_ACTIONS[i]!;
}

export function triagePatch(action: TriageAction, captureLabel?: CaptureIssueLabel): UpdateReviewRequest {
  const triage = { key: "triage", value: action };
  switch (action) {
    case "confirm":
      return { status: "done", decision: { action }, labels: [triage] };
    case "capture_issue":
      if (!isCaptureIssueLabel(captureLabel)) throw new Error("capture_issue needs a capture issue label");
      return {
        status: "done",
        decision: { action, capture_issue: captureLabel },
        labels: [triage, { key: "capture_issue", value: captureLabel }],
      };
    case "exclude_from_norms":
      return {
        status: "done",
        decision: { action, exclude_from_norms: true },
        labels: [triage, { key: "exclude_from_norms", value: true }],
      };
    case "dismiss":
      return { status: "dismissed", decision: { action }, labels: [triage] };
  }
}
