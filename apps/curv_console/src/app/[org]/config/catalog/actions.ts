"use server";

import { revalidatePath } from "next/cache";
import { z } from "@/lib/zod";
import type { CatalogFailure, CatalogResult } from "@/components/catalog/types";
import { ApiError } from "@/lib/api";
import { stimulusCatalogSchema, type ConfigTransition } from "@/lib/api/catalog";
import { authedApi } from "@/lib/auth/server";
import { pathsFor } from "@/lib/paths";

function failure(err: unknown): CatalogFailure {
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

const draftInput = z.object({
  version: z.string().max(200),
  notes: z.string().max(2000).optional(),
  body: stimulusCatalogSchema,
});

const base = (orgId: string) => pathsFor(orgId).config.catalog;

/** POST /v1/config/stimulus_catalog. The body is checked for shape here; the backend validates it fully. */
export async function createCatalogDraftAction(orgId: string, input: unknown): Promise<CatalogResult<{ version: string }>> {
  const parsed = draftInput.safeParse(input);
  if (!parsed.success) return { ok: false, slug: "validation-failed", status: 400 };
  const { version, notes, body } = parsed.data;
  try {
    const created = await (await authedApi()).createConfigDocument("stimulus_catalog", {
      version: version.trim(),
      body,
      ...(notes?.trim() ? { notes: notes.trim() } : {}),
    });
    revalidatePath(base(orgId));
    return { ok: true, version: created.version };
  } catch (err) {
    return failure(err);
  }
}

const STEPS: readonly ConfigTransition[] = ["approve", "publish", "retire"];

/** POST /v1/config/stimulus_catalog/{version}/approve|publish|retire (superadmin, four-eyes rule on the server). */
export async function transitionCatalogAction(
  orgId: string,
  version: string,
  step: ConfigTransition,
): Promise<CatalogResult<{ status: string }>> {
  if (!STEPS.includes(step)) return { ok: false, slug: "validation-failed", status: 400 };
  try {
    const doc = await (await authedApi()).transitionConfigDocument("stimulus_catalog", version, step);
    revalidatePath(base(orgId));
    revalidatePath(pathsFor(orgId).config.docVersion("catalog", version));
    return { ok: true, status: doc.status };
  } catch (err) {
    return failure(err);
  }
}
