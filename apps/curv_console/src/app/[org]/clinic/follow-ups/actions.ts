"use server";

import { revalidatePath } from "next/cache";
import type { ActionResult } from "@/components/participant/types";
import { ApiError } from "@/lib/api";
import { authedApi } from "@/lib/auth/server";
import { pathsFor } from "@/lib/paths";

/**
 * Mark one follow-up done from the list: `FollowUp` carries no row version, so read the visit first and send its
 * version with the `PATCH` (`412 stale-version` if someone changed it in between).
 */
export async function markFollowUpDoneAction(orgId: string, encounterId: string): Promise<ActionResult> {
  try {
    const api = await authedApi();
    const e = await api.getEncounter(encounterId);
    await api.updateEncounter(encounterId, e.row_version, { follow_up_done_at: new Date().toISOString() });
    revalidatePath(pathsFor(orgId).clinic.followUps);
    return { ok: true };
  } catch (err) {
    if (err instanceof ApiError) return { ok: false, slug: err.slug, status: err.status };
    return { ok: false, slug: null, status: 0 };
  }
}
