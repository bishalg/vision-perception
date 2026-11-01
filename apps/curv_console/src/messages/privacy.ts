/** User-visible strings for erasure, retention and research datasets (WP-B.9). en only; merged by import. */
import type { ErasureStatus } from "@/lib/api/privacy";

const errors: Record<string, string> = {
  "validation-failed": "Some fields were rejected.",
  forbidden: "Your role cannot do this.",
  "not-found": "Not found.",
  conflict: "That action is not possible in the current state.",
  "rate-limited": "Too many requests. Wait a moment and try again.",
  unavailable: "Could not reach the server. Try again shortly.",
  fallback: "The request failed. Try again.",
};

export function privacyErrorMessage(slug: string | null, status: number): string {
  if (slug && errors[slug]) return errors[slug];
  if (status === 403) return errors.forbidden!;
  if (status === 404) return errors["not-found"]!;
  if (status === 429) return errors["rate-limited"]!;
  if (status === 0) return errors.unavailable!;
  return errors.fallback!;
}

export const privacyText = {
  nav: { erasure: "Erasure requests", retention: "Data retention" },
  denied: "Your role cannot view this page.",
  erasure: {
    title: "Erasure requests",
    intro:
      "Requests to erase a participant's data, filed by a device, a user or the system. Consent is withdrawn at once; the erasure runs as a background job.",
    cols: {
      participant: "Participant",
      status: "Status",
      requested: "Requested",
      by: "Requested by",
      completed: "Completed",
      counts: "What was removed",
    },
    statuses: {
      pending: "Pending",
      completed: "Completed",
      held: "Held",
    } satisfies Record<ErasureStatus, string>,
    statusHelp: {
      pending: "Filed; the erasure job has not finished.",
      completed: "Everything about the participant was erased.",
      held: "Kept under clinical record hold",
    } satisfies Record<ErasureStatus, string>,
    heldExplain:
      "Held means kept under clinical record hold: the participant has signed clinical records, so those records, the sessions they document and the patient reference stay until the owner decides on retention versus erasure. Everything else was erased and the participant is withdrawn.",
    holdReasons: { clinical_record: "Kept under clinical record hold" } as Record<string, string>,
    requestedBy: { device: "Device", user: "Staff user", system: "System" } as Record<string, string>,
    counts: {
      sessions_deleted: (n: number) => `${n} sessions deleted`,
      sessions_held: (n: number) => `${n} sessions held`,
      care_events_deleted: (n: number) => `${n} care events deleted`,
      session_blobs_deleted: (n: number) => `${n} files deleted`,
      export_artifacts_deleted: (n: number) => `${n} export files deleted`,
      clinical_drafts_deleted: (n: number) => `${n} clinical drafts deleted`,
      patient_ref_deleted: "Patient reference deleted",
    },
    noCounts: "Not run yet",
    empty: "No erasure requests.",
    next: "Next page",
    first: "First page",
    showing: (n: number) => `${n} requests shown`,
  },
  participantAction: {
    cols: "Actions",
    open: "Request erasure",
    title: "Request erasure",
    confirm: (token: string) =>
      `Erase all data of participant ${token}? Consent is withdrawn now and the data is deleted by a background job. This cannot be undone. Signed clinical records are kept under clinical record hold.`,
    confirmButton: "Yes, request erasure",
    cancel: "Cancel",
    queued: "Erasure requested. Follow it under Erasure requests.",
    duplicate: "An erasure request for this participant is already open. Nothing new was filed.",
    viewRequests: "View erasure requests",
    failed: "The erasure was not requested.",
  },
  retention: {
    title: "Data retention",
    intro:
      "The retention sweep deletes the frame recordings of sessions that are past the organisation's retention period. Aggregate counts are kept. Preview first: a dry run counts what would be deleted and deletes nothing.",
    preview: "Preview (dry run)",
    run: "Run sweep",
    runHint: "Run sweep deletes frame recordings that are past retention. It cannot be undone.",
    confirm: (n: number | null) =>
      n == null
        ? "Delete the frame recordings that are past retention?"
        : `Delete the frame recordings of ${n} sessions that are past retention?`,
    confirmButton: "Yes, run sweep",
    cancel: "Cancel",
    starting: "Starting...",
    queued: "Job queued.",
    running: "Counting...",
    runningReal: "Sweeping...",
    failedJob: "The job failed.",
    resultDry: "Preview",
    resultReal: "Sweep done",
    cols: { org: "Organisation", months: "Retention (months)", cutoff: "Sessions received before", purged: "Frame recordings" },
    purgedDry: (n: number) => `${n} would be deleted`,
    purgedReal: (n: number) => `${n} deleted`,
    noOrgs: "Nothing is past retention.",
    failed: "The sweep was not started.",
  },
  dataset: {
    labels: "Include reviewer labels",
    labelsHint: "Adds each session's reviewer labels to the dataset.",
    deidentifiedOnly: "Datasets are always de-identified and use sessions with research consent. Identified mode is not available for them.",
    destinationsOff: "Datasets are downloaded; they cannot be sent to a destination.",
    withLabels: "With labels",
    withoutLabels: "No labels",
  },
};
