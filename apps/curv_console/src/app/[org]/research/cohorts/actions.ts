"use server";

/** Server actions of the cohort pages (WP-X.7, RUI-5, COH-1). The backend validates the definition again and audits the write. */
import { revalidatePath } from "next/cache";
import { z } from "@/lib/zod";
import { ApiError } from "@/lib/api";
import { cohortDefinitionSchema } from "@/lib/api/research";
import { authedApi } from "@/lib/auth/server";
import type { ResearchFailure, ResearchResult } from "@/lib/research/result";

function failure(err: unknown): ResearchFailure {
  if (err instanceof ApiError) {
    return { ok: false, slug: err.slug, status: err.status, ...(err.problem?.detail ? { detail: err.problem.detail } : {}) };
  }
  return { ok: false, slug: null, status: 0 };
}

const invalid: ResearchFailure = { ok: false, slug: "validation-failed", status: 400 };
const cohortsPath = (org: string) => `/${encodeURIComponent(org)}/research/cohorts`;
const createInput = z.object({ name: z.string().trim().min(1).max(200), definition: cohortDefinitionSchema });

/** `POST /v1/orgs/{id}/cohorts`; `409 conflict` when the name is taken. */
export async function createCohortAction(orgId: string, input: unknown): Promise<ResearchResult<{ id: string }>> {
  const parsed = createInput.safeParse(input);
  if (!parsed.success) return invalid;
  try {
    const created = await (await authedApi()).createCohort(orgId, parsed.data);
    revalidatePath(cohortsPath(orgId));
    return { ok: true, id: created.id };
  } catch (err) {
    return failure(err);
  }
}

/** `PATCH /v1/cohorts/{id}` with `archived: true`. */
export async function archiveCohortAction(orgId: string, cohortId: string): Promise<ResearchResult> {
  if (!z.string().min(1).max(200).safeParse(cohortId).success) return invalid;
  try {
    await (await authedApi()).updateCohort(cohortId, { archived: true });
    revalidatePath(cohortsPath(orgId));
    return { ok: true };
  } catch (err) {
    return failure(err);
  }
}
