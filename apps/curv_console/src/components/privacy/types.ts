import { privacyErrorMessage } from "@/messages/privacy";

/** Failure outcome of a privacy server action: slug, status and field reasons only. */
export type PrivacyFailure = { ok: false; slug: string | null; status: number; params?: string };
export type PrivacyResult<T = object> = ({ ok: true } & T) | PrivacyFailure;

export function privacyFailureText(f: PrivacyFailure): string {
  return f.params ? `${privacyErrorMessage(f.slug, f.status)} ${f.params}` : privacyErrorMessage(f.slug, f.status);
}
