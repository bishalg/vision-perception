"use server";

import { revalidatePath } from "next/cache";
import type { PrivacyFailure, PrivacyResult } from "@/components/privacy/types";
import { ApiError } from "@/lib/api";
import {
  isJobFinished,
  retentionSweepResultSchema,
  type RetentionSweepResult,
} from "@/lib/api/privacy";
import { authedApi } from "@/lib/auth/server";
import { pathsFor } from "@/lib/paths";

function failure(err: unknown): PrivacyFailure {
  if (err instanceof ApiError) {
    const params = err.problem?.invalid_params?.map((p) => `${p.name}: ${p.reason}`).join("; ");
    return { ok: false, slug: err.slug, status: err.status, ...(params ? { params } : {}) };
  }
  return { ok: false, slug: null, status: 0 };
}

/** POST /v1/orgs/{id}/participants/{participant_id}/erasure: 202 (new) or 200 with `duplicate: true`. */
export async function requestErasureAction(
  orgId: string,
  participantId: string,
): Promise<PrivacyResult<{ duplicate: boolean }>> {
  try {
    const r = await (await authedApi()).requestParticipantErasure(orgId, participantId);
    revalidatePath(pathsFor(orgId).clinic.participants);
    revalidatePath(pathsFor(orgId).data.erasure);
    return { ok: true, duplicate: r.duplicate === true };
  } catch (err) {
    return failure(err);
  }
}

export type SweepPoll =
  | { state: "pending"; status: string }
  | { state: "done"; result: RetentionSweepResult }
  | { state: "failed" };

/** POST /v1/orgs/{id}/retention-sweep. */
export async function startSweepAction(orgId: string, dryRun: boolean): Promise<PrivacyResult<{ jobId: string }>> {
  try {
    const job = await (await authedApi()).startRetentionSweep(orgId, { dry_run: dryRun });
    return { ok: true, jobId: job.job_id };
  } catch (err) {
    return failure(err);
  }
}

/** GET /v1/jobs/{id}: one poll of a retention sweep job. */
export async function pollSweepAction(jobId: string): Promise<PrivacyResult<{ poll: SweepPoll }>> {
  try {
    const job = await (await authedApi()).getJob(jobId);
    if (!isJobFinished(job)) return { ok: true, poll: { state: "pending", status: job.status } };
    const parsed = job.status === "succeeded" ? retentionSweepResultSchema.safeParse(job.result) : null;
    return { ok: true, poll: parsed?.success ? { state: "done", result: parsed.data } : { state: "failed" } };
  } catch (err) {
    return failure(err);
  }
}
