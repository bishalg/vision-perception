"use server";

import { redirect } from "next/navigation";
import { ApiError, getApi } from "@/lib/api";
import { safeNextPath } from "@/lib/auth/guard";
import { clearSession, readRefreshToken, writeSession } from "@/lib/auth/server";

export interface LoginState {
  error: "failed" | "unavailable" | null;
}

export async function loginAction(_prev: LoginState, formData: FormData): Promise<LoginState> {
  const email = String(formData.get("email") ?? "").trim();
  const password = String(formData.get("password") ?? "");
  const next = safeNextPath(String(formData.get("next") ?? ""));
  if (!email || !password) return { error: "failed" };

  let target: string;
  try {
    const pair = await getApi().login(email, password);
    await writeSession(pair);
    target = next ?? `/${encodeURIComponent(pair.user.org_id)}/overview`;
  } catch (err) {
    if (err instanceof ApiError && (err.status === 400 || err.status === 401 || err.status === 403 || err.status === 422)) {
      return { error: "failed" };
    }
    return { error: "unavailable" };
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
