"use server";

import { revalidatePath } from "next/cache";
import { ApiError } from "@/lib/api";
import { authedApi } from "@/lib/auth/server";
import type { EnrolResult } from "@/components/EnrolCodePanel";

function message(err: unknown): string {
  if (err instanceof ApiError) {
    const params = err.problem?.invalid_params?.map((p) => `${p.name}: ${p.reason}`).join("; ");
    return params || err.slug || `status ${err.status}`;
  }
  return "unavailable";
}

function optionalInt(value: FormDataEntryValue | null): number | undefined {
  const s = typeof value === "string" ? value.trim() : "";
  if (!s) return undefined;
  const n = Number(s);
  // Let the backend reject out-of-range values; only refuse what is not a whole number at all.
  return Number.isInteger(n) ? n : Number.NaN;
}

/** POST /v1/orgs/{id}/devices/enrol-code. The plain code goes straight back to the browser and is never stored. */
export async function createEnrolCodeAction(orgId: string, formData: FormData): Promise<EnrolResult> {
  const label = String(formData.get("label") ?? "").trim();
  const maxUses = optionalInt(formData.get("max_uses"));
  const days = optionalInt(formData.get("expires_in_days"));
  if (Number.isNaN(maxUses) || Number.isNaN(days)) return { ok: false, error: "whole numbers only" };
  try {
    const api = await authedApi();
    const res = await api.createEnrolCode(orgId, {
      ...(label ? { label } : {}),
      ...(maxUses !== undefined ? { max_uses: maxUses } : {}),
      ...(days !== undefined ? { expires_in_days: days } : {}),
    });
    return { ok: true, code: res.code, max_uses: res.max_uses, expires_at: res.expires_at };
  } catch (err) {
    return { ok: false, error: message(err) };
  }
}

/** POST /v1/devices/{id}/revoke (idempotent). */
export async function revokeDeviceAction(orgId: string, deviceId: string): Promise<{ ok: boolean }> {
  try {
    const api = await authedApi();
    // The backend checks that the device belongs to the caller's org (404 otherwise).
    await api.revokeDevice(deviceId);
    revalidatePath(`/${encodeURIComponent(orgId)}/devices`);
    revalidatePath(`/${encodeURIComponent(orgId)}/overview`);
    return { ok: true };
  } catch {
    return { ok: false };
  }
}
