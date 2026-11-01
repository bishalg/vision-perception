import { exportsErrorMessage } from "@/messages/exports";

/** Failure outcome of an exports or integrations server action: slug, status and field reasons only. */
export type ActionFailure = { ok: false; slug: string | null; status: number; params?: string };

export type ActionResult<T = object> = ({ ok: true } & T) | ActionFailure;

export function failureText(f: ActionFailure): string {
  return f.params ? `${exportsErrorMessage(f.slug, f.status)} ${f.params}` : exportsErrorMessage(f.slug, f.status);
}
