"use server";

/**
 * WP-C.15 TOTP enrolment. The password, the codes and the pending seed only pass through these actions: they are
 * sent to the backend and never logged, cached or stored by the console. The seed goes back to the browser once,
 * for display.
 */
import { ApiError } from "@/lib/api";
import { authedApi } from "@/lib/auth/server";
import type { TotpFailure, TotpResult } from "@/components/security/types";

function failure(err: unknown): TotpFailure {
  if (err instanceof ApiError) return { ok: false, slug: err.slug, status: err.status };
  return { ok: false, slug: null, status: 0 };
}

const field = (fd: FormData, k: string) => String(fd.get(k) ?? "");
const code = (fd: FormData) => field(fd, "totp_code").replace(/\s+/g, "");

/** POST /v1/users/me/totp/enrol. */
export async function enrolTotpAction(formData: FormData): Promise<TotpResult<{ secret: string; otpauth_url: string; expires_at: string }>> {
  try {
    const res = await (await authedApi()).enrolTotp(field(formData, "password"));
    return { ok: true, secret: res.secret, otpauth_url: res.otpauth_url, expires_at: res.expires_at };
  } catch (err) {
    return failure(err);
  }
}

/** POST /v1/users/me/totp/confirm. */
export async function confirmTotpAction(formData: FormData): Promise<TotpResult<{ totp_enrolled: boolean }>> {
  try {
    const res = await (await authedApi()).confirmTotp(code(formData));
    return { ok: true, totp_enrolled: res.totp_enrolled };
  } catch (err) {
    return failure(err);
  }
}

/** DELETE /v1/users/me/totp (password and code). */
export async function disableTotpAction(formData: FormData): Promise<TotpResult<{ totp_enrolled: boolean }>> {
  try {
    const totpCode = code(formData);
    const res = await (await authedApi()).disableTotp(field(formData, "password"), totpCode || undefined);
    return { ok: true, totp_enrolled: res.totp_enrolled };
  } catch (err) {
    return failure(err);
  }
}
