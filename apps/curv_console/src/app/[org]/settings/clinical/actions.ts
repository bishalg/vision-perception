"use server";

import { revalidatePath } from "next/cache";
import type { LanguageResult } from "@/components/security/types";
import { ApiError } from "@/lib/api";
import { authedApi } from "@/lib/auth/server";
import { isLocale } from "@/lib/i18n/types";

/**
 * I18N-20: `PATCH /v1/orgs/{id}` `{ default_locale }` (owner and admin; the backend enforces it). The layout is
 * revalidated so every user without an own setting gets the new language on the next render.
 */
export async function saveOrgLanguageAction(orgId: string, formData: FormData): Promise<LanguageResult> {
  const raw = String(formData.get("default_locale") ?? "");
  if (!isLocale(raw)) return { ok: false, slug: "validation-failed", status: 400 };
  try {
    await (await authedApi()).updateOrgClinicalSettings(orgId, {}, { default_locale: raw });
    revalidatePath("/", "layout");
    return { ok: true };
  } catch (err) {
    if (err instanceof ApiError) return { ok: false, slug: err.slug, status: err.status };
    return { ok: false, slug: null, status: 0 };
  }
}
