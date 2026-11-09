"use server";

import { revalidatePath } from "next/cache";
import type { ActionFailure, ActionResult } from "@/components/exports/types";
import { ApiError } from "@/lib/api";
import {
  analysisExportSchema,
  exportFormatSchema,
  exportModeSchema,
  exportPurposeSchema,
  type ExportFilters,
  type ExportRequest,
} from "@/lib/api/exports";
import { isDatasetFormat } from "@/lib/api/exports";
import { SHARE_LINK_DAY_CHOICES, SHARE_LINK_DOWNLOAD_CHOICES, type CreatedShareLink } from "@/lib/api/share";
import { authedApi } from "@/lib/auth/server";

function failure(err: unknown): ActionFailure {
  if (err instanceof ApiError) {
    const params = err.problem?.invalid_params?.map((p) => `${p.name}: ${p.reason}`).join("; ");
    return { ok: false, slug: err.slug, status: err.status, ...(params ? { params } : {}) };
  }
  return { ok: false, slug: null, status: 0 };
}

const text = (fd: FormData, k: string) => String(fd.get(k) ?? "").trim();

/** Ids separated by commas or whitespace. The backend validates them; this only splits. */
function list(fd: FormData, k: string): string[] {
  return text(fd, k).split(/[\s,]+/).filter(Boolean);
}

/** Only fields the user filled in are sent (an empty filter selects every session). */
function filtersFrom(fd: FormData): ExportFilters {
  const f: ExportFilters = {};
  for (const k of ["from", "to", "flag", "quality", "bundle_version"] as const) if (text(fd, k)) f[k] = text(fd, k);
  const states = fd.getAll("clinical_state").map(String).filter(Boolean);
  if (states.length) f.clinical_state = states;
  const reviewed = text(fd, "reviewed");
  if (reviewed === "yes") f.reviewed = true;
  if (reviewed === "no") f.reviewed = false;
  const sessions = list(fd, "session_ids");
  if (sessions.length) f.session_ids = sessions;
  const participants = list(fd, "participant_ids");
  if (participants.length) f.participant_ids = participants;
  return f;
}

/** POST /v1/orgs/{id}/exports. The backend decides what the consent purpose and the role allow. */
export async function createExportAction(orgId: string, formData: FormData): Promise<ActionResult<{ id: string }>> {
  const format = exportFormatSchema.safeParse(text(formData, "format"));
  const mode = exportModeSchema.safeParse(text(formData, "mode"));
  const purpose = exportPurposeSchema.safeParse(text(formData, "purpose"));
  // Datasets send no mode or purpose (the disabled selects are not submitted).
  // WP-B.9: datasets go through the dedicated endpoint (de-identified, research, no destinations).
  if (format.success && isDatasetFormat(format.data)) {
    try {
      const created = await (await authedApi()).createDatasetExport({
        format: format.data === "dataset_jsonl" ? "jsonl" : "parquet",
        reason: text(formData, "reason"),
        filters: filtersFrom(formData),
        include_labels: formData.get("include_labels") !== null,
      });
      revalidatePath(`/${encodeURIComponent(orgId)}/exports`);
      return { ok: true, id: created.id };
    } catch (err) {
      return failure(err);
    }
  }
  // RUN-7: the results of one analysis run, always de-identified, for research or quality improvement only.
  if (format.success && format.data === "analysis_results") {
    const analysis = analysisExportSchema.safeParse({
      run_id: text(formData, "analysis_run_id"),
      layout: text(formData, "analysis_layout"),
      encoding: text(formData, "analysis_encoding"),
    });
    const researchPurpose = purpose.success && (purpose.data === "research" || purpose.data === "quality_improvement");
    if (!analysis.success || !analysis.data.run_id || !researchPurpose || !purpose.success) {
      return { ok: false, slug: "validation-failed", status: 400 };
    }
    try {
      const created = await (await authedApi()).createExport(orgId, {
        format: "analysis_results",
        mode: "deidentified",
        purpose: purpose.data,
        reason: text(formData, "reason"),
        filters: {},
        analysis: analysis.data,
      });
      revalidatePath(`/${encodeURIComponent(orgId)}/exports`);
      return { ok: true, id: created.id };
    } catch (err) {
      return failure(err);
    }
  }
  if (!format.success || !mode.success || !purpose.success) {
    return { ok: false, slug: "validation-failed", status: 400 };
  }
  const destinations = formData.getAll("destination_ids").map(String).filter(Boolean);
  const request: ExportRequest = {
    format: format.data,
    mode: mode.data,
    purpose: purpose.data,
    reason: text(formData, "reason"),
    filters: filtersFrom(formData),
    ...(destinations.length ? { destination_ids: destinations } : {}),
  };
  try {
    const created = await (await authedApi()).createExport(orgId, request);
    revalidatePath(`/${encodeURIComponent(orgId)}/exports`);
    return { ok: true, id: created.id };
  } catch (err) {
    return failure(err);
  }
}

const intIn = (fd: FormData, k: string, choices: readonly number[]): number | undefined => {
  const n = Number.parseInt(text(fd, k), 10);
  return choices.includes(n) ? n : undefined;
};

/**
 * WP-C.19 (SHR-8): `POST /v1/exports/{id}/share-link`. The passcode in the answer goes to the panel that shows it
 * once; it is not logged or revalidated into page data (the list route never returns it).
 */
export async function createShareLinkAction(
  orgId: string,
  exportId: string,
  formData: FormData,
): Promise<ActionResult<{ created: CreatedShareLink }>> {
  try {
    const created = await (await authedApi()).createShareLink(exportId, {
      expires_in_days: intIn(formData, "expires_in_days", SHARE_LINK_DAY_CHOICES) ?? null,
      max_downloads: intIn(formData, "max_downloads", SHARE_LINK_DOWNLOAD_CHOICES) ?? null,
    });
    revalidatePath(`/${encodeURIComponent(orgId)}/exports/${encodeURIComponent(exportId)}`);
    return { ok: true, created };
  } catch (err) {
    return failure(err);
  }
}

/** WP-C.19 (SHR-8): `DELETE /v1/share-links/{id}`. */
export async function revokeShareLinkAction(orgId: string, exportId: string, linkId: string): Promise<ActionResult> {
  try {
    await (await authedApi()).revokeShareLink(linkId);
    revalidatePath(`/${encodeURIComponent(orgId)}/exports/${encodeURIComponent(exportId)}`);
    return { ok: true };
  } catch (err) {
    return failure(err);
  }
}
