import type { ExportFilters } from "@/lib/api/exports";

const text = (fd: FormData, k: string) => String(fd.get(k) ?? "").trim();

/** Only the four audit filters (HIP-3); the backend validates them and refuses any other. */
export function auditExportFilters(fd: FormData): ExportFilters {
  const f: ExportFilters = {};
  for (const k of ["from", "to", "actor_id"] as const) if (text(fd, k)) f[k] = text(fd, k);
  if (text(fd, "action_prefix")) f.action_prefix = text(fd, "action_prefix");
  return f;
}
