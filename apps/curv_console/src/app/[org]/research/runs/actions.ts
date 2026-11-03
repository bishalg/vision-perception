"use server";

/**
 * Server actions of the run pages (WP-X.7, RUI-6). Each validates the id and hands it to the API client; the backend
 * decides whether the role may cancel or re-run and whether the run is still cancellable. There is no job endpoint in
 * the client, so the status poll reads the run itself (`GET /v1/analysis-runs/{id}`); the backend may audit each read.
 */
import { revalidatePath } from "next/cache";
import { z } from "@/lib/zod";
import { ApiError } from "@/lib/api";
import { authedApi } from "@/lib/auth/server";
import { snapshotOf, type RunSnapshot } from "@/lib/research/runs";
import type { ResearchFailure, ResearchResult } from "@/lib/research/result";

function failure(err: unknown): ResearchFailure {
  if (err instanceof ApiError) {
    return { ok: false, slug: err.slug, status: err.status, ...(err.problem?.detail ? { detail: err.problem.detail } : {}) };
  }
  return { ok: false, slug: null, status: 0 };
}

const invalid: ResearchFailure = { ok: false, slug: "validation-failed", status: 400 };
const id = z.string().min(1).max(200);
const runsPath = (org: string) => `/${encodeURIComponent(org)}/research/runs`;

/** One status poll. The run must belong to the org in the URL. */
export async function pollRunAction(orgId: string, runId: string): Promise<ResearchResult<{ run: RunSnapshot }>> {
  if (!id.safeParse(runId).success) return invalid;
  try {
    const run = await (await authedApi()).getAnalysisRun(runId);
    if (run.org_id !== orgId) return { ok: false, slug: "not-found", status: 404 };
    return { ok: true, run: snapshotOf(run) };
  } catch (err) {
    return failure(err);
  }
}

/** `POST /v1/analysis-runs/{id}/cancel`; `409 job-finished` when it already ended. */
export async function cancelRunAction(orgId: string, runId: string): Promise<ResearchResult<{ run: RunSnapshot }>> {
  if (!id.safeParse(runId).success) return invalid;
  try {
    const run = await (await authedApi()).cancelAnalysisRun(runId);
    revalidatePath(runsPath(orgId));
    return { ok: true, run: snapshotOf(run) };
  } catch (err) {
    return failure(err);
  }
}

/** `POST /v1/analysis-runs/{id}/rerun`: the same definition again; answers the new run's id. */
export async function rerunRunAction(orgId: string, runId: string): Promise<ResearchResult<{ runId: string }>> {
  if (!id.safeParse(runId).success) return invalid;
  try {
    const run = await (await authedApi()).rerunAnalysisRun(runId);
    revalidatePath(runsPath(orgId));
    return { ok: true, runId: run.id };
  } catch (err) {
    return failure(err);
  }
}
