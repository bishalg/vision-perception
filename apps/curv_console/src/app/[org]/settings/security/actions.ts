"use server";

/**
 * WP-C.15 TOTP enrolment. The password, the codes and the pending seed only pass through these actions: they are
 * sent to the backend and never logged, cached or stored by the console. The seed goes back to the browser once,
 * for display.
 */
import { revalidatePath } from "next/cache";
import { ApiError } from "@/lib/api";
import { authedApi } from "@/lib/auth/server";
import type { LanguageResult, TotpFailure, TotpResult } from "@/components/security/types";
import { isLocale } from "@/lib/i18n/types";

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
    // The nudge and the required-state panel come from `GET /users/me`: render them again.
    revalidatePath("/", "layout");
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
    revalidatePath("/", "layout");
    return { ok: true, totp_enrolled: res.totp_enrolled };
  } catch (err) {
    return failure(err);
  }
}

/**
 * I18N-20: `PATCH /v1/users/me` `{ locale }`. An empty choice is "follow the organisation" (`null`); anything that is
 * not a console locale is refused here (the backend would answer 400 `validation-failed`). The layout is revalidated so
 * the next render, including `<html lang>`, uses the new language.
 */
export async function saveLanguageAction(formData: FormData): Promise<LanguageResult> {
  const raw = field(formData, "locale");
  if (raw !== "" && !isLocale(raw)) return { ok: false, slug: "validation-failed", status: 400 };
  try {
    await (await authedApi()).updateMyProfile({ locale: raw === "" ? null : (raw as "en" | "ne") });
    revalidatePath("/", "layout");
    return { ok: true };
  } catch (err) {
    return failure(err);
  }
}
