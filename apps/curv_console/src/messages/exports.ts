/** User-visible strings for exports and integration settings (WP-I.8). Merged next to `en` by import, not by editing it. */
import type {
  DeliveryEvent,
  DeliveryStatus,
  DeliveryTrigger,
  ExportFormat,
  ExportMode,
  ExportPurpose,
  ExportStatus,
  IntegrationEnvironment,
  IntegrationKind,
  IntegrationStatus,
} from "@/lib/api/exports";

const errors: Record<string, string> = {
  "validation-failed": "Some fields were rejected.",
  "consent-purpose-mismatch": "That purpose does not allow this mode. Identified exports are limited to clinical care and patient requests.",
  "patient-ref-disabled": "HL7 exports need the organisation's MRN mapping. Turn it on in clinical settings first.",
  "export-expired": "The export has expired. Request a new one.",
  "export-too-large": "The export is too large. Narrow the filters.",
  "host-key-mismatch": "The server's host key does not match the pinned fingerprint.",
  "case-consent-missing": "An identified case package needs the consent reference and the date consent was recorded.",
  "participant-withdrawn": "This participant has withdrawn. Their case cannot be shared.",
  "share-link-expired": "This share link has expired.",
  "share-link-exhausted": "This share link has reached its download limit.",
  "clinical-crypto-unavailable": "Secret storage is not configured on the server.",
  conflict: "That action is not possible in the current state.",
  forbidden: "Your role cannot do this.",
  "not-found": "Not found.",
  "rate-limited": "Too many requests. Wait a moment and try again.",
  unavailable: "Could not reach the server. Try again shortly.",
  fallback: "The request failed. Try again.",
};

export function exportsErrorMessage(slug: string | null, status: number): string {
  if (slug && errors[slug]) return errors[slug];
  if (status === 403) return errors.forbidden!;
  if (status === 404) return errors["not-found"]!;
  if (status === 429) return errors["rate-limited"]!;
  if (status === 0) return errors.unavailable!;
  return errors.fallback!;
}

const formats: Record<ExportFormat, string> = {
  csv: "CSV (ZIP)",
  csv_excel: "CSV for Excel (ZIP)",
  csv_sheets: "CSV for Sheets (ZIP)",
  xlsx: "Excel workbook",
  pdf_summary: "PDF summary (not available yet)",
  fhir_bundle: "FHIR bundle (one session)",
  fhir_ndjson: "FHIR NDJSON",
  hl7v2: "HL7 v2 message (one session)",
  dataset_jsonl: "Research dataset (JSONL)",
  dataset_parquet: "Research dataset (Parquet)",
  case_package: "Case package (ZIP)",
  audit_csv: "Audit log (CSV)",
  analysis_results: "Analysis results (one run)",
};

/**
 * The pre-filled export of an analysis run (RUN-7; opened from the run page's "Export results..."). `exports.ts`
 * is English-only elsewhere; this block carries `ne` too (`// ne: draft` until WP-L.3 reviews it).
 */
export interface AnalysisExportMessages {
  title: string;
  run: string;
  layout: string;
  encoding: string;
  layouts: Record<"aggregates" | "sessions", string>;
  encodings: Record<"jsonl" | "csv" | "parquet", string>;
  parquetNote: string;
  purposeOnly: string;
  deidentifiedOnly: string;
  fixedFormat: string;
}

export const analysisExportMessages: { en: AnalysisExportMessages; ne: AnalysisExportMessages } = {
  en: {
    title: "Analysis run",
    run: "Run",
    layout: "Table",
    encoding: "File type",
    layouts: { aggregates: "Aggregates (summary tables)", sessions: "Per-session rows" },
    encodings: { jsonl: "JSONL", csv: "CSV", parquet: "Parquet" },
    parquetNote: "Parquet is available for per-session rows only.",
    purposeOnly: "Analysis results are exported for research or quality improvement only.",
    deidentifiedOnly: "Analysis results are always de-identified.",
    fixedFormat: "The format is fixed to the results of this run.",
  },
  // ne: draft
  ne: {
    title: "विश्लेषण रन",
    run: "रन",
    layout: "तालिका",
    encoding: "फाइल प्रकार",
    layouts: { aggregates: "सङ्कलित तालिका (सारांश)", sessions: "सत्र अनुसार पङ्क्ति" },
    encodings: { jsonl: "JSONL", csv: "CSV", parquet: "Parquet" },
    parquetNote: "Parquet सत्र अनुसार पङ्क्तिका लागि मात्र उपलब्ध छ।",
    purposeOnly: "विश्लेषण नतिजा अनुसन्धान वा गुणस्तर सुधारका लागि मात्र निर्यात गरिन्छ।",
    deidentifiedOnly: "विश्लेषण नतिजा सधैं पहिचान हटाइएको हुन्छ।",
    fixedFormat: "ढाँचा यही रनको नतिजामा तय गरिएको छ।",
  },
};

const sharedExportText = {
  formats,
  modes: { identified: "Identified", deidentified: "De-identified" } satisfies Record<ExportMode, string>,
  purposes: {
    clinical_care: "Clinical care",
    research: "Research",
    quality_improvement: "Quality improvement",
    patient_request: "Patient request",
  } satisfies Record<ExportPurpose, string>,
  statuses: {
    queued: "Queued",
    running: "Running",
    succeeded: "Ready",
    failed: "Failed",
    expired: "Expired",
  } satisfies Record<ExportStatus, string>,
};

export const exportsText = {
  ...sharedExportText,
  title: "Exports",
  intro: "Export session data as files. Each export is audited with its reason, and the file is deleted after it expires.",
  denied: "Your role cannot view exports.",
  cols: {
    created: "Requested",
    format: "Format",
    mode: "Mode",
    purpose: "Purpose",
    status: "Status",
    sessions: "Sessions",
    excluded: "Excluded",
    expires: "Expires",
    actions: "Actions",
  },
  empty: "No exports yet.",
  details: "Details",
  download: "Download",
  downloadHint: "The download link is valid for five minutes and is recorded in the audit log.",
  next: "Next page",
  first: "First page",
  showing: (n: number, total: number | null | undefined) =>
    total == null ? `Showing ${n} exports` : `Showing ${n} of ${total} exports`,
  excludedReasons: {
    withdrawn: "Withdrawn",
    no_research_consent: "No research consent",
    mode_not_allowed: "Mode not allowed",
    capture_source_refused: "Capture source refused",
  } as Record<string, string>,
  excludedNone: "None",
  create: {
    title: "New export",
    readOnly: "Your role cannot create exports.",
    format: "Format",
    purpose: "Purpose",
    mode: "Mode",
    purposeHint: "Research uses only sessions whose participant consented to research. Withdrawn participants are always left out.",
    reason: "Reason (10 to 500 characters, kept in the audit log)",
    filters: "Filters (all optional)",
    from: "Started from",
    to: "Started until",
    flag: "Attention flag",
    quality: "Quality",
    clinicalState: "Clinical review state",
    reviewed: "Review queue",
    reviewedAny: "Any",
    reviewedYes: "Reviewed",
    reviewedNo: "Not reviewed",
    anyOption: "Any",
    bundleVersion: "Config bundle version",
    sessionIds: "Session ids (one per line or comma separated; FHIR bundle and HL7 need exactly one)",
    participantIds: "Participant ids (one per line or comma separated)",
    destinations: "Also deliver to",
    destinationsHint: "Leave all unchecked to download only.",
    submit: "Request export",
    queued: "Export queued.",
    view: "View export",
    failed: "The export was not created.",
    flags: { green: "Green", yellow: "Yellow", red: "Red", unreliable: "Unreliable" } as Record<string, string>,
    qualities: { optimal: "Optimal", acceptable: "Acceptable", degraded: "Degraded", rejected: "Rejected" } as Record<string, string>,
    clinicalStates: {
      none: "No review",
      open: "Claimed",
      in_progress: "In progress",
      awaiting_cosign: "Awaiting cosign",
      final: "Final",
      amending: "Amending",
    } as Record<string, string>,
  },
  detail: {
    back: "All exports",
    summary: "Summary",
    id: "Export id",
    reason: "Reason",
    filters: "Filters",
    noFilters: "None (every session of the organisation)",
    participant: "Participant",
    openParticipant: "Open the participant",
    file: "File",
    size: "Size",
    sha256: "SHA-256",
    error: "Error",
    completed: "Completed",
    destinations: "Destinations",
    deliveries: "Deliveries",
    noDeliveries: "No deliveries for this export.",
  },
};

export const deliveriesText = {
  cols: {
    created: "Queued",
    destination: "Destination",
    export: "Export",
    event: "Event",
    status: "Status",
    attempts: "Attempts",
    error: "Error class",
    ref: "Reference",
    delivered: "Delivered",
  },
  statuses: {
    pending: "Pending",
    sending: "Sending",
    delivered: "Delivered",
    failed: "Failed",
  } satisfies Record<DeliveryStatus, string>,
  events: {
    "export.ready": "Export ready",
    "review.finalized": "Review finalized",
  } satisfies Record<DeliveryEvent, string>,
  empty: "No deliveries yet.",
  next: "Next page",
  first: "First page",
  showing: (n: number, total: number | null | undefined) =>
    total == null ? `Showing ${n} deliveries` : `Showing ${n} of ${total} deliveries`,
};

export const integrationsText = {
  title: "Integrations",
  intro: "Destinations that receive exports: signed webhooks and SFTP drops. Secrets are write-only and are never shown again.",
  denied: "Your role cannot view integrations.",
  readOnly: "Your role can view integrations but not change them.",
  kinds: {
    webhook: "Webhook",
    sftp: "SFTP",
    fhir_server: "FHIR server",
    hl7_mllp: "HL7 (MLLP)",
  } satisfies Record<IntegrationKind, string>,
  environments: { test: "Test", production: "Production" } satisfies Record<IntegrationEnvironment, string>,
  statuses: { active: "Active", disabled: "Disabled" } satisfies Record<IntegrationStatus, string>,
  triggers: { manual: "Manual", on_final_sign: "On final sign" } satisfies Record<DeliveryTrigger, string>,
  cols: {
    name: "Name",
    kind: "Kind",
    environment: "Environment",
    status: "Status",
    credential: "Credential",
    lastTest: "Last test",
    actions: "Actions",
  },
  empty: "No integrations yet.",
  manage: "Manage",
  back: "All integrations",
  credentialSet: "Set",
  credentialMissing: "Missing",
  testOk: "Passed",
  testFailed: "Failed",
  testNever: "Never run",
  form: {
    createTitle: "Add a destination",
    editTitle: "Settings",
    name: "Name",
    kind: "Kind",
    environment: "Environment",
    environmentHint: "Only production destinations receive automatic deliveries.",
    status: "Status",
    url: "Receiver URL (https only)",
    events: "Events",
    host: "Host",
    port: "Port",
    username: "Username",
    remoteDir: "Remote directory (absolute)",
    hostKey: "Host key fingerprint (SHA256:...)",
    filePrefix: "File name prefix",
    trigger: "Delivery trigger",
    format: "Automatic delivery format",
    mode: "Automatic delivery mode",
    unset: "Not set",
    create: "Add destination",
    save: "Save settings",
    created: "Destination added.",
    saved: "Settings saved.",
    failed: "Could not save.",
    openCreated: "Open it to set up its credential.",
  },
  secret: {
    title: "Signing secret",
    intro: "Deliveries are signed with this secret so the receiver can verify them. It cannot be read back.",
    present: (fp: string | null | undefined) => (fp ? `A secret is set (fingerprint ${fp}).` : "A secret is set."),
    absent: "No secret is set. Deliveries and tests will fail until you set one.",
    custom: "Use your own secret (whsec_ and base64, leave empty to generate one)",
    set: "Set secret",
    rotate: "Rotate secret",
    generatedTitle: "New secret",
    generatedOnce: "Copy it now. It is shown only once and cannot be recovered.",
    saved: "Secret saved.",
    failed: "Could not save the secret.",
  },
  ssh: {
    title: "SSH key",
    intro: "Install this public key for the SFTP user at the hospital. The private key stays on the server.",
    none: "No key has been generated yet.",
    publicKey: "Public key",
    fingerprint: "Fingerprint",
    generate: "Generate key",
    rotate: "Rotate key",
    rotateWarn: "Rotating replaces the key: the hospital must install the new one before deliveries work again.",
    generated: "New key generated.",
    failed: "Could not generate the key.",
  },
  test: {
    title: "Connection test",
    intro: "Sends a synthetic record (participant TEST-0000, no real data).",
    run: "Send test",
    ok: "The test delivery succeeded.",
    failed: "The test delivery failed.",
    errorClass: "Error class",
    ref: "Reference",
    at: "Tested",
    unavailable: "The test could not be run.",
  },
  deliveries: { title: "Delivery log" },
  next: "Next page",
  first: "First page",
  showing: (n: number, total: number | null | undefined) =>
    total == null ? `Showing ${n} destinations` : `Showing ${n} of ${total} destinations`,
  delete: {
    title: "Delete destination",
    intro: "Removes this destination from the list and from export requests. Its delivery log is kept. This cannot be undone.",
    open: "Delete destination",
    confirm: (name: string) => `Delete "${name}"? Exports can no longer be delivered to it.`,
    confirmButton: "Yes, delete",
    cancel: "Cancel",
    inFlight: "Deliveries to this destination are still pending. Wait until they are delivered or failed, or disable the destination instead.",
    failed: "Could not delete the destination.",
    deleted: "Destination deleted.",
  },
};

/** WP-C.19 (SHR-10): the "Share case" dialog on the participant workspace. */
export const shareCaseText = {
  open: "Share case",
  title: "Share this case",
  intro:
    "Creates one case package (a ZIP with the sessions, a FHIR bundle and a summary) as an export. Once it is ready, create a share link for the recipient on the export page.",
  mode: "Mode",
  modeHint: "De-identified packages carry pseudonyms only. Identified packages carry the participant token and need recorded consent.",
  purpose: "Purpose",
  purposeHint: "Research uses only sessions whose participant consented to research.",
  recipient: "Recipient (name and organisation, 10 to 200 characters)",
  recipientHint: "Kept encrypted on the server and printed in the package's manifest.",
  reason: "Reason (10 to 500 characters, kept in the audit log)",
  consentReference: "Consent reference (for example the consent form number)",
  consentDate: "Date consent was recorded",
  consentHint: "Required for an identified package.",
  attachments: "Include attachments",
  sessions: "Sessions",
  sessionsAll: "All of this participant's sessions",
  sessionsPick: "Choose sessions",
  sessionsLoading: "Loading sessions...",
  sessionsNone: "No sessions to choose from.",
  sessionsFailed: "Could not load the sessions.",
  noRecall:
    "Once the recipient has downloaded the package it cannot be recalled. Erasing the participant later revokes open links and deletes unexpired files only.",
  submit: "Create case package",
  submitting: "Creating...",
  cancel: "Cancel",
  close: "Close",
  created: "Case package requested.",
  view: "Open the export to share it",
  failed: "The case package was not created.",
};

/** WP-C.19 (SHR-8, SHR-10): share links of a case-package export, on the export detail page. */
export const shareLinksText = {
  title: "Share links",
  intro:
    "A share link lets one recipient without a console account download this package after entering a passcode. Every open and download is recorded with the recipient's IP address.",
  denied: "Your role cannot create or revoke share links.",
  notReady: "Share links can be created once the package is ready.",
  expires: "Link valid for",
  days: (n: number) => (n === 1 ? "1 day" : `${n} days`),
  downloads: "Downloads allowed",
  create: "Create share link",
  creating: "Creating...",
  createdTitle: "Share link created",
  url: "Link",
  passcode: "Passcode",
  copy: "Copy",
  copied: "Copied",
  copyFailed: "Copy failed: select the text and copy it by hand.",
  once: "The passcode is shown only now and cannot be recovered. Copy it before you close this box.",
  otherChannel: "Send the passcode by another channel than the link (for example the link by e-mail and the passcode by phone or text message).",
  done: "I have copied it, close",
  failed: "The share link was not created.",
  empty: "No share links yet.",
  cols: {
    created: "Created",
    state: "State",
    downloads: "Downloads",
    opens: "Opens",
    lastOpened: "Last opened",
    expires: "Expires",
    actions: "Actions",
  },
  states: { active: "Active", revoked: "Revoked", expired: "Expired", exhausted: "Used up" } as Record<string, string>,
  revoke: "Revoke",
  revokeConfirm: "Revoke this link? The recipient can no longer open it. Downloads already made cannot be recalled.",
  revokeYes: "Yes, revoke",
  cancel: "Cancel",
  revokeFailed: "The link was not revoked.",
  revoked: "Link revoked.",
};

/** WP-C.19 (SHR-9, SHR-10): the public `/share/[id]` page a recipient opens. No organisation or case detail on it. */
export const sharePageText = {
  title: "Shared file",
  heading: "Download a shared file",
  intro: "A clinic shared a file with you through this link. Enter the passcode you received separately to download it.",
  passcode: "Passcode",
  passcodeHint: "12 letters and digits, as you received it.",
  open: "Continue",
  opening: "Checking...",
  file: "File",
  size: "Size",
  expires: "Link valid until",
  downloadsLeft: "Downloads left",
  download: "Download",
  downloadHint: "The download button works for five minutes. After that, enter the passcode again.",
  started: "The download has started. To download again, enter the passcode again.",
  again: "Enter the passcode again",
  notice:
    "This file may contain health information. Store it securely and use it only for the purpose it was shared for. Do not try to re-identify a de-identified package.",
  errors: {
    "share-passcode-invalid": "The passcode is not correct. Check it and try again.",
    "share-link-expired": "This link has expired. Ask the clinic for a new one.",
    "share-link-exhausted": "This link has reached its download limit. Ask the clinic for a new one.",
    "export-expired": "The shared file is no longer available. Ask the clinic to share it again.",
    gone: "This link does not exist or has been withdrawn.",
    token: "The download button has expired. Enter the passcode again.",
    "rate-limited": "Too many attempts. Wait a while and try again.",
    fallback: "Something went wrong. Try again later.",
  } as Record<string, string>,
  retryAfter: (s: number) => (s >= 120 ? `Try again in about ${Math.ceil(s / 60)} minutes.` : `Try again in ${s} seconds.`),
};

/**
 * The public page's error key for a backend answer: the slug when it is one the recipient may see, else one generic
 * key per status. Unknown and revoked links are both `404` (`gone`); the page reveals nothing beyond that.
 */
export function shareErrorKey(slug: string | null, status: number): string {
  if (slug && Object.hasOwn(sharePageText.errors, slug)) return slug;
  if (status === 404) return "gone";
  if (status === 401 || status === 403) return "token";
  if (status === 429) return "rate-limited";
  return "fallback";
}
