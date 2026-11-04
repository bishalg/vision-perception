import type { Locale } from "@/lib/i18n/types";
import type { ClinicalNote } from "@/lib/api/participant";
import { participantErrorMessage } from "@/messages/participant";

/** Failure outcome of a participant server action: slug, status and field reasons only (never a typed value). */
export type ActionFailure = { ok: false; slug: string | null; status: number; params?: string };
export type ActionResult<T = object> = ({ ok: true } & T) | ActionFailure;

/** The sentence for a failed action in the user's locale (`useText().locale`). */
export function failureText(f: ActionFailure, locale: Locale): string {
  const base = participantErrorMessage(f.slug, f.status, locale);
  return f.params ? `${base} ${f.params}` : base;
}

/** What a server action returns for a note create; the page re-renders from the revalidated list. */
export type NoteActionResult = ActionResult<{ note: ClinicalNote }>;
/** The revealed MRN lives only in client state for as long as the panel shows it. */
export type RevealResult = ActionResult<{ authority: string; mrn: string; updatedAt: string }>;
