"use server";

import { redirect } from "next/navigation";
import { ApiError, getApi } from "@/lib/api";
import { safeNextPath } from "@/lib/auth/guard";
import { clearSession, readRefreshToken, writeSession } from "@/lib/auth/server";
import { pathsFor } from "@/lib/paths";

export interface LoginState {
  error: "failed" | "unavailable" | "totp-invalid" | "rate-limited" | "server-problem" | null;
  /** `totp`: the password was accepted and a one-time code is needed; the form resubmits email+password+code. */
  step?: "password" | "totp";
  /** Seconds from `Retry-After` on a 429. */
  retryAfter?: number | null;
}

export async function loginAction(_prev: LoginState, formData: FormData): Promise<LoginState> {
  const email = String(formData.get("email") ?? "").trim();
  const password = String(formData.get("password") ?? "");
  const next = safeNextPath(String(formData.get("next") ?? ""));
  // The code field is only rendered on the TOTP step, so its presence says which step was submitted.
  const totpField = formData.get("totp_code");
  const step: "password" | "totp" = totpField === null ? "password" : "totp";
  const totpCode = String(totpField ?? "").replace(/\s+/g, "");
  if (!email || !password) return { error: "failed", step: "password" };
  if (step === "totp" && !/^\d{6}$/.test(totpCode)) return { error: "totp-invalid", step };

  let target: string;
  try {
    const pair = await getApi().login(email, password, step === "totp" ? totpCode : undefined);
    await writeSession(pair);
    const org = pair.user.org_id;
    // The superadmin has no home org: without `next` it lands on `/`, which routes by the org cookie.
    target = next ?? (org === null ? "/" : pathsFor(org).clinic.overview);
  } catch (err) {
    if (err instanceof ApiError) {
      if (err.status === 401 && err.slug === "totp-required") return { error: null, step: "totp" };
      if (err.status === 401 && err.slug === "totp-invalid") return { error: "totp-invalid", step: "totp" };
      if (err.status === 429) return { error: "rate-limited", step, retryAfter: err.retryAfter };
      if (err.status === 503 && err.slug === "clinical-crypto-unavailable") return { error: "server-problem", step };
    }
    if (err instanceof ApiError && (err.status === 400 || err.status === 401 || err.status === 403 || err.status === 422)) {
      return { error: "failed", step: "password" };
    }
    return { error: "unavailable", step };
  }
  redirect(target);
}

export async function logoutAction(): Promise<void> {
  const refresh = await readRefreshToken();
  if (refresh) {
    try {
      await getApi().logout(refresh);
    } catch {
      // Best effort: cookies are cleared regardless.
    }
  }
  await clearSession();
  redirect("/login");
}
