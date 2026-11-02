"use server";

import { revalidatePath } from "next/cache";
import { ApiError, roleSchema, userStatusSchema } from "@/lib/api";
import { authedApi, currentRole } from "@/lib/auth/server";
import { can } from "@/lib/permissions";
import type { TotpResetResult, UserActionResult } from "@/components/UsersAdmin";

function failure(err: unknown): UserActionResult {
  if (err instanceof ApiError) {
    const params = err.problem?.invalid_params?.map((p) => `${p.name}: ${p.reason}`).join("; ");
    return { ok: false, slug: err.slug ?? null, status: err.status, params };
  }
  return { ok: false, slug: null, status: 0 };
}

const text = (fd: FormData, key: string) => String(fd.get(key) ?? "").trim();

/** POST /v1/orgs/{id}/users. The backend authorises and validates; the checks here only avoid pointless calls. */
export async function createUserAction(orgId: string, formData: FormData): Promise<UserActionResult> {
  const role = roleSchema.safeParse(text(formData, "role"));
  if (!role.success) return { ok: false, slug: "validation-failed", status: 400, params: "role: choose a role" };
  if (!can(await currentRole(), "user.manage")) return { ok: false, slug: "forbidden", status: 403 };
  try {
    const api = await authedApi();
    const user = await api.createUser(orgId, {
      email: text(formData, "email"),
      role: role.data,
      // Passwords are not trimmed: they are exactly what the actor typed.
      password: String(formData.get("password") ?? ""),
      ...(text(formData, "display_name") ? { display_name: text(formData, "display_name") } : {}),
      ...(text(formData, "professional_registration")
        ? { professional_registration: text(formData, "professional_registration") }
        : {}),
    });
    revalidatePath(`/${encodeURIComponent(orgId)}/users`);
    return { ok: true, email: user.email };
  } catch (err) {
    return failure(err);
  }
}

/**
 * POST /v1/orgs/{id}/users/{user_id}/totp/reset (TRS-1, UI-3). The caller's password and code arrive in this
 * `FormData` only, are sent once and never returned, logged or stored; the result carries a slug, not the input.
 */
export async function resetTotpAction(orgId: string, userId: string, formData: FormData): Promise<TotpResetResult> {
  if (!can(await currentRole(), "user.totp_reset")) return { ok: false, slug: "forbidden", status: 403 };
  const code = text(formData, "totp_code");
  try {
    await (await authedApi()).resetUserTotp(orgId, userId, {
      // Passwords are not trimmed: they are exactly what the actor typed.
      password: String(formData.get("password") ?? ""),
      ...(code ? { totp_code: code } : {}),
      reason: text(formData, "reason"),
    });
    revalidatePath(`/${encodeURIComponent(orgId)}/users`);
    return { ok: true };
  } catch (err) {
    if (err instanceof ApiError) {
      return { ok: false, slug: err.slug, status: err.status, ...(err.retryAfter != null ? { retryAfter: err.retryAfter } : {}) };
    }
    return { ok: false, slug: null, status: 0 };
  }
}

/** PATCH /v1/orgs/{id}/users/{user_id}: role and status only (profile fields are the user's own, via /users/me). */
export async function updateUserAction(orgId: string, userId: string, formData: FormData): Promise<UserActionResult> {
  const role = roleSchema.safeParse(text(formData, "role"));
  // An empty status leaves it unchanged (invited users cannot be set back to invited).
  const rawStatus = text(formData, "status");
  const status = rawStatus ? userStatusSchema.safeParse(rawStatus) : null;
  if (!role.success || (status && !status.success)) return { ok: false, slug: "validation-failed", status: 400 };
  if (!can(await currentRole(), "user.manage")) return { ok: false, slug: "forbidden", status: 403 };
  try {
    const api = await authedApi();
    const user = await api.updateUser(orgId, userId, { role: role.data, ...(status?.success ? { status: status.data } : {}) });
    revalidatePath(`/${encodeURIComponent(orgId)}/users`);
    return { ok: true, email: user.email };
  } catch (err) {
    return failure(err);
  }
}
