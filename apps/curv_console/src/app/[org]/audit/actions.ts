"use server";

import { redirect } from "next/navigation";
import { ApiError } from "@/lib/api";
import type { ExportFilters, ExportRequest } from "@/lib/api/exports";
import { authedApi, currentRole } from "@/lib/auth/server";
import { can } from "@/lib/permissions";
import { auditExportFilters } from "./filters";

const text = (fd: FormData, k: string) => String(fd.get(k) ?? "").trim();

/** `/{org}/audit?...` with the current filters kept, so a failed export leaves the page as it was. */
function backTo(orgId: string, filters: ExportFilters, error: string): string {
  const p = new URLSearchParams();
  if (filters.actor_id) p.set("actor_id", filters.actor_id);
  if (filters.action_prefix) p.set("action", filters.action_prefix);
  if (filters.from) p.set("from", filters.from);
  if (filters.to) p.set("to", filters.to);
  p.set("export_error", error);
  return `/${encodeURIComponent(orgId)}/audit?${p.toString()}`;
}

/**
 * HIP-3: `POST /v1/orgs/{id}/exports` with `audit_csv`, de-identified, quality improvement, submitted through the
 * existing `createExport`. The backend decides who may ask (owner and admin); the page only shows the control to
 * them. On success the person lands on the export, where it is downloaded like any other (audited).
 */
export async function exportAuditCsvAction(orgId: string, formData: FormData): Promise<void> {
  const filters = auditExportFilters(formData);
  const role = await currentRole();
  if (!can(role, "audit.export") || !can(role, "export.create")) redirect(backTo(orgId, filters, "forbidden"));
  const request: ExportRequest = {
    format: "audit_csv",
    mode: "deidentified",
    purpose: "quality_improvement",
    reason: text(formData, "reason"),
    filters,
  };
  let id: string;
  try {
    id = (await (await authedApi()).createExport(orgId, request)).id;
  } catch (err) {
    redirect(backTo(orgId, filters, err instanceof ApiError && err.slug ? err.slug : "unavailable"));
  }
  redirect(`/${encodeURIComponent(orgId)}/exports/${encodeURIComponent(id)}`);
}
