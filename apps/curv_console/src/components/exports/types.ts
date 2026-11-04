import { exportsErrorMessage } from "@/messages/exports";

/** Failure outcome of an exports or integrations server action: slug, status and field reasons only. */
export type ActionFailure = { ok: false; slug: string | null; status: number; params?: string };

export type ActionResult<T = object> = ({ ok: true } & T) | ActionFailure;

/** `errors` is `t.exports.errors` of the active locale (default: English). */
export function failureText(f: ActionFailure, errors?: Record<string, string>): string {
  const message = exportsErrorMessage(f.slug, f.status, errors);
  return f.params ? `${message} ${f.params}` : message;
}
