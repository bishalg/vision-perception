/** User-visible strings for erasure, retention and research datasets (WP-B.9). en only; merged by import. */
import type { ErasureStatus } from "@/lib/api/privacy";
import type { Widen } from "@/lib/i18n/types";

const errors: Record<string, string> = {
  "validation-failed": "Some fields were rejected.",
  forbidden: "Your role cannot do this.",
  "not-found": "Not found.",
  conflict: "That action is not possible in the current state.",
  "rate-limited": "Too many requests. Wait a moment and try again.",
  unavailable: "Could not reach the server. Try again shortly.",
  fallback: "The request failed. Try again.",
};

/** `table` is `t.privacy.errors` of the active locale (default: English, for callers outside this package). */
export function privacyErrorMessage(slug: string | null, status: number, table: Record<string, string> = errors): string {
  if (slug && table[slug]) return table[slug];
  if (status === 403) return table.forbidden!;
  if (status === 404) return table["not-found"]!;
  if (status === 429) return table["rate-limited"]!;
  if (status === 0) return table.unavailable!;
  return table.fallback!;
}

export const privacyText = {
  errors,
  pagination: "Pagination",
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

/** WP-L.1.0 (I18N-15): both locales of this file. `ne` is typed by the `en` tree; the wave package replaces the copy. */
const en = privacyText;
// ne: draft (machine draft; WP-L.3 reviews it)
const ne: Widen<typeof en> = {
  errors: {
    "validation-failed": "केही फिल्ड स्वीकार गरिएनन्।",
    forbidden: "तपाईंको भूमिकाले यो गर्न मिल्दैन।",
    "not-found": "फेला परेन।",
    conflict: "अहिलेको अवस्थामा यो काम गर्न मिल्दैन।",
    "rate-limited": "धेरै अनुरोध भयो। केही बेर पर्खनुहोस् र फेरि प्रयास गर्नुहोस्।",
    unavailable: "सर्भरमा पुग्न सकिएन। केही बेरपछि फेरि प्रयास गर्नुहोस्।",
    fallback: "अनुरोध असफल भयो। फेरि प्रयास गर्नुहोस्।",
  },
  pagination: "पृष्ठ सूची",
  nav: { erasure: "मेटाउने अनुरोध", retention: "डेटा राख्ने अवधि" },
  denied: "तपाईंको भूमिकाले यो पृष्ठ हेर्न मिल्दैन।",
  erasure: {
    title: "मेटाउने अनुरोध",
    intro:
      "सहभागीको डेटा मेटाउने अनुरोध, जुन उपकरण, प्रयोगकर्ता वा प्रणालीले पेश गरेको हुन्छ। सहमति तुरुन्तै फिर्ता हुन्छ; मेटाउने काम पृष्ठभूमिमा चल्छ।",
    cols: {
      participant: "सहभागी",
      status: "अवस्था",
      requested: "अनुरोध गरिएको",
      by: "अनुरोध गर्ने",
      completed: "सकिएको",
      counts: "के हटाइयो",
    },
    statuses: {
      pending: "बाँकी",
      completed: "सकियो",
      held: "रोकिएको",
    },
    statusHelp: {
      pending: "पेश गरिएको छ; मेटाउने काम सकिएको छैन।",
      completed: "सहभागीको सबै डेटा मेटियो।",
      held: "क्लिनिकल अभिलेख रोकमा राखिएको",
    },
    heldExplain:
      "रोकिएको भनेको क्लिनिकल अभिलेख रोकमा राखिएको हो: सहभागीको हस्ताक्षर गरिएका क्लिनिकल अभिलेख छन्, त्यसैले ती अभिलेख, तिनले वर्णन गर्ने सत्र र बिरामी सन्दर्भ मालिकले राख्ने वा मेटाउने निर्णय नगरेसम्म रहन्छन्। बाँकी सबै मेटियो र सहभागीले सहमति फिर्ता लिएको मानिन्छ।",
    holdReasons: { clinical_record: "क्लिनिकल अभिलेख रोकमा राखिएको" },
    requestedBy: { device: "उपकरण", user: "कर्मचारी प्रयोगकर्ता", system: "प्रणाली" },
    counts: {
      sessions_deleted: (n: number) => `${n} सत्र मेटिए`,
      sessions_held: (n: number) => `${n} सत्र रोकिए`,
      care_events_deleted: (n: number) => `${n} हेरचाह घटना मेटिए`,
      session_blobs_deleted: (n: number) => `${n} फाइल मेटिए`,
      export_artifacts_deleted: (n: number) => `${n} निर्यात फाइल मेटिए`,
      clinical_drafts_deleted: (n: number) => `${n} क्लिनिकल मस्यौदा मेटिए`,
      patient_ref_deleted: "बिरामी सन्दर्भ मेटियो",
    },
    noCounts: "अझै चलेको छैन",
    empty: "मेटाउने अनुरोध छैन।",
    next: "अर्को पृष्ठ",
    first: "पहिलो पृष्ठ",
    showing: (n: number) => `${n} अनुरोध देखाइएको`,
  },
  participantAction: {
    cols: "कार्य",
    open: "मेटाउन अनुरोध गर्नुहोस्",
    title: "मेटाउन अनुरोध गर्नुहोस्",
    confirm: (token: string) =>
      `सहभागी ${token} को सबै डेटा मेटाउने? सहमति अहिले फिर्ता हुन्छ र डेटा पृष्ठभूमि कामले मेटाउँछ। यो फिर्ता गर्न मिल्दैन। हस्ताक्षर गरिएका क्लिनिकल अभिलेख क्लिनिकल अभिलेख रोकमा राखिन्छन्।`,
    confirmButton: "हो, मेटाउन अनुरोध गर्नुहोस्",
    cancel: "रद्द गर्नुहोस्",
    queued: "मेटाउन अनुरोध गरियो। “मेटाउने अनुरोध” मा हेर्नुहोस्।",
    duplicate: "यो सहभागीका लागि मेटाउने अनुरोध पहिल्यै खुला छ। नयाँ अनुरोध पेश गरिएन।",
    viewRequests: "मेटाउने अनुरोध हेर्नुहोस्",
    failed: "मेटाउने अनुरोध गरिएन।",
  },
  retention: {
    title: "डेटा राख्ने अवधि",
    intro:
      "राख्ने अवधिको सफाइले संस्थाको राख्ने अवधि नाघेका सत्रका फ्रेम रेकर्डिङ मेटाउँछ। जोडिएका सङ्ख्या रहन्छन्। पहिले पूर्वावलोकन गर्नुहोस्: परीक्षण सफाइले के मेटिन्थ्यो गन्छ तर केही मेटाउँदैन।",
    preview: "पूर्वावलोकन (परीक्षण)",
    run: "सफाइ चलाउनुहोस्",
    runHint: "सफाइ चलाउँदा राख्ने अवधि नाघेका फ्रेम रेकर्डिङ मेटिन्छन्। यो फिर्ता गर्न मिल्दैन।",
    confirm: (n: number | null) =>
      n == null
        ? "राख्ने अवधि नाघेका फ्रेम रेकर्डिङ मेटाउने?"
        : `राख्ने अवधि नाघेका ${n} सत्रका फ्रेम रेकर्डिङ मेटाउने?`,
    confirmButton: "हो, सफाइ चलाउनुहोस्",
    cancel: "रद्द गर्नुहोस्",
    starting: "सुरु गर्दै...",
    queued: "काम लाइनमा राखियो।",
    running: "गन्दै...",
    runningReal: "सफा गर्दै...",
    failedJob: "काम असफल भयो।",
    resultDry: "पूर्वावलोकन",
    resultReal: "सफाइ सकियो",
    cols: { org: "संस्था", months: "राख्ने अवधि (महिना)", cutoff: "यस अघि प्राप्त सत्र", purged: "फ्रेम रेकर्डिङ" },
    purgedDry: (n: number) => `${n} मेटिने थिए`,
    purgedReal: (n: number) => `${n} मेटिए`,
    noOrgs: "राख्ने अवधि नाघेको केही छैन।",
    failed: "सफाइ सुरु गरिएन।",
  },
  dataset: {
    labels: "समीक्षकका लेबल समावेश गर्नुहोस्",
    labelsHint: "डेटासेटमा हरेक सत्रका समीक्षक लेबल थप्छ।",
    deidentifiedOnly: "डेटासेट सधैं पहिचान हटाइएको हुन्छ र अनुसन्धान सहमति भएका सत्र मात्र प्रयोग गर्छ। यिनका लागि पहिचानसहित मोड उपलब्ध छैन।",
    destinationsOff: "डेटासेट डाउनलोड गरिन्छ; गन्तव्यमा पठाउन मिल्दैन।",
    withLabels: "लेबलसहित",
    withoutLabels: "लेबल बिना",
  },
};
export const privacyMessages = { en, ne } as const;
