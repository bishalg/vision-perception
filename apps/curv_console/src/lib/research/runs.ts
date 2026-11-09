/** The part of an analysis run that changes while it executes: what the status panel polls (WP-X.7). */
import type { AnalysisRun, JobProgress, RunStatus } from "@/lib/api/research";
import { pathsFor } from "@/lib/paths";

export interface RunSnapshot {
  status: RunStatus;
  progress: JobProgress | null;
  error: string | null;
}

export function snapshotOf(run: Pick<AnalysisRun, "status" | "progress" | "error">): RunSnapshot {
  return { status: run.status, progress: run.progress ?? null, error: run.error ?? null };
}

/** `/{org}/exports?format=analysis_results&run={id}`: the export form with the run pre-filled (RUN-7). */
export function exportPrefillHref(orgId: string, runId: string): string {
  return `${pathsFor(orgId).data.exports}?format=analysis_results&run=${encodeURIComponent(runId)}`;
}
