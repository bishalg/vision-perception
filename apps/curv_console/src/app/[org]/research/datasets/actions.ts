"use server";

/**
 * Server actions of the dataset pages (WP-X.11, ADR-0015 R-7). Each checks the input's shape and hands it to the API
 * client; the backend checks every rule again, freezes the members, computes the content hash and audits the write.
 */
import { revalidatePath } from "next/cache";
import { z } from "@/lib/zod";
import { ApiError } from "@/lib/api";
import { createDatasetRequestSchema } from "@/lib/api/research";
import { authedApi } from "@/lib/auth/server";
import { pathsFor } from "@/lib/paths";
import type { ResearchFailure, ResearchResult } from "@/lib/research/result";

function failure(err: unknown): ResearchFailure {
  if (err instanceof ApiError) {
    return {
      ok: false,
      slug: err.slug,
      status: err.status,
      ...(err.problem?.detail ? { detail: err.problem.detail } : {}),
      ...(err.problem?.invalid_params?.length ? { params: err.problem.invalid_params } : {}),
    };
  }
  return { ok: false, slug: null, status: 0 };
}

const invalid: ResearchFailure = { ok: false, slug: "validation-failed", status: 400 };
const datasetsPath = (org: string) => pathsFor(org).research.datasets;
const createInput = createDatasetRequestSchema.extend({
  name: z.string().trim().min(1).max(80),
  cohort_id: z.string().min(1).max(200),
});

/** `POST /v1/research/datasets`; `409 conflict` when the name is taken. */
export async function createDatasetAction(orgId: string, input: unknown): Promise<ResearchResult<{ id: string }>> {
  const parsed = createInput.safeParse(input);
  if (!parsed.success) return invalid;
  try {
    const created = await (await authedApi()).createDataset(orgId, parsed.data);
    revalidatePath(datasetsPath(orgId));
    return { ok: true, id: created.id };
  } catch (err) {
    return failure(err);
  }
}

/** `POST /v1/research/datasets/{id}/archive`. */
export async function archiveDatasetAction(orgId: string, datasetId: string): Promise<ResearchResult> {
  if (!z.string().min(1).max(200).safeParse(datasetId).success) return invalid;
  try {
    await (await authedApi()).archiveDataset(datasetId);
    revalidatePath(datasetsPath(orgId));
    return { ok: true };
  } catch (err) {
    return failure(err);
  }
}
