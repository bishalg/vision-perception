import { privacyErrorMessage } from "@/messages/privacy";

/** Failure outcome of a privacy server action: slug, status and field reasons only. */
export type PrivacyFailure = { ok: false; slug: string | null; status: number; params?: string };
export type PrivacyResult<T = object> = ({ ok: true } & T) | PrivacyFailure;

/** `errors` is `t.privacy.errors` of the active locale (default: English). */
export function privacyFailureText(f: PrivacyFailure, errors?: Record<string, string>): string {
  const message = privacyErrorMessage(f.slug, f.status, errors);
  return f.params ? `${message} ${f.params}` : message;
}
