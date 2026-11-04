import { attentionFlagSchema } from "@/lib/api";
import { TIMELINE_KINDS, type TimelineEntry, type TimelineKind } from "@/lib/api/participant";
import { messagesFor, type Messages } from "@/lib/i18n/messages";
import { formatFileSize, labelOr } from "@/messages/participant";

/** Parse repeated `?kind=` params into a validated, de-duplicated list (unknown values are dropped). */
export function parseKinds(value: string | string[] | undefined): TimelineKind[] {
  const raw = Array.isArray(value) ? value : value ? [value] : [];
  return TIMELINE_KINDS.filter((k) => raw.includes(k));
}

const DATE_RE = /^\d{4}-\d{2}-\d{2}$/;

/** A `?from=` / `?to=` calendar day (`YYYY-MM-DD`); anything else is dropped. */
export function parseDay(value: string | string[] | undefined): string | undefined {
  const v = Array.isArray(value) ? value[0] : value;
  return v && DATE_RE.test(v) ? v : undefined;
}

/** The attention flag of a session entry when its summary carries a valid one. */
export function flagOf(e: TimelineEntry) {
  const parsed = e.kind === "session" ? attentionFlagSchema.safeParse(e.summary.attention_flag) : null;
  return parsed?.success ? parsed.data : null;
}

/**
 * Kind-specific header text in the bundle's locale (English without a bundle). Headers carry no free text (CW-3), so
 * nothing here can leak note content. Enumerations come from the domain catalogue (`enumLabel`).
 */
export function describeEntry(e: TimelineEntry, t: Messages = messagesFor("en")): string {
  const tl = t.participant.timeline;
  const { locale } = t;
  const { enumLabel } = t.domain;
  const s = e.summary;
  const str = (k: string) => (typeof s[k] === "string" ? (s[k] as string) : "");
  switch (e.kind) {
    case "session":
      return str("attention_flag")
        ? tl.flag(labelOr(locale, "attention_flag", str("attention_flag")).toLowerCase())
        : enumLabel(locale, "timeline_entry_kind", "session");
    case "clinical_review": {
      const state = str("state") || "open";
      return tl.review(tl.reviewStates[state] ?? state.replace(/_/g, " "), s.version_no);
    }
    case "note":
      return tl.note(str("note_kind") === "correction");
    case "letter": {
      const kind = labelOr(locale, "letter_kind", str("letter_kind") === "referral" ? "referral" : "summary");
      const status = typeof s.status === "string" ? labelOr(locale, "letter_status", s.status).toLowerCase() : null;
      return tl.withStatus(kind, status);
    }
    case "care_event": {
      const kind = str("care_kind");
      return tl.care(kind ? (tl.careKinds[kind] ?? kind) : tl.careUnknown);
    }
    // REC-5 headers as B.19 writes them (`read/timeline.rs`): `{encounter_kind, status}` (the clinician is the "By"
    // column) and `{attachment_kind, file_name, byte_size}`. Never the visit summary or reason.
    case "encounter": {
      const kind = str("encounter_kind");
      const label = kind ? labelOr(locale, "encounter_kind", kind) : enumLabel(locale, "timeline_entry_kind", "encounter");
      const status =
        s.status === "closed" || s.status === "open" ? enumLabel(locale, "encounter_status", s.status).toLowerCase() : null;
      return tl.withStatus(label, status);
    }
    case "attachment": {
      const kind = str("attachment_kind");
      return tl.attachment(
        kind ? labelOr(locale, "attachment_kind", kind) : tl.fileFallback,
        str("file_name"),
        typeof s.byte_size === "number" ? formatFileSize(s.byte_size) : "",
      );
    }
  }
}

/** The workspace tab an entry opens, for kinds that have one. */
export function entryTab(kind: TimelineKind): "visits" | "attachments" | null {
  return kind === "encounter" ? "visits" : kind === "attachment" ? "attachments" : null;
}
