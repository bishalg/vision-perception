"use server";

/**
 * Server actions of the research area (WP-X.5, RUI-1..5). Each validates its input for shape and hands it to the API
 * client; the backend decides what the role may do and runs the Rust checks again. Nothing here computes.
 */
import { revalidatePath } from "next/cache";
import { z } from "@/lib/zod";
import { ApiError } from "@/lib/api";
import {
  analysisRunRequestSchema,
  cohortDefinitionSchema,
  type CohortPreview,
} from "@/lib/api/research";
import { authedApi } from "@/lib/auth/server";
import type { ResearchFailure, ResearchResult } from "@/lib/research/result";
import { pathsFor } from "@/lib/paths";

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
const base = (org: string) => pathsFor(org).research.root;
const id = z.string().min(1).max(200);
const profileDoc = z.record(z.string(), z.unknown());

const createInput = z.object({
  name: z.string().trim().min(1).max(200),
  description: z.string().trim().max(2000).optional(),
  from_experiment_id: id.optional(),
});

/** POST /v1/orgs/{id}/experiments (`ExperimentWrite`); `409 experiment-name-exists`. */
export async function createExperimentAction(orgId: string, input: unknown): Promise<ResearchResult<{ id: string }>> {
  const parsed = createInput.safeParse(input);
  if (!parsed.success) return invalid;
  try {
    const created = await (await authedApi()).createExperiment(orgId, {
      name: parsed.data.name,
      ...(parsed.data.description ? { description: parsed.data.description } : {}),
      ...(parsed.data.from_experiment_id ? { from_experiment_id: parsed.data.from_experiment_id } : {}),
    });
    revalidatePath(pathsFor(orgId).research.experiments);
    return { ok: true, id: created.id };
  } catch (err) {
    return failure(err);
  }
}

const updateInput = z.object({
  name: z.string().trim().min(1).max(200),
  description: z.string().trim().max(2000).nullable(),
  profile: profileDoc,
});

/** PATCH /v1/experiments/{id} with `If-Match`; `412 stale-version`, `400 config-invalid` (field reasons returned). */
export async function updateExperimentAction(
  orgId: string,
  experimentId: string,
  rowVersion: number,
  input: unknown,
): Promise<ResearchResult<{ rowVersion: number; profileHash: string }>> {
  const parsed = updateInput.safeParse(input);
  if (!parsed.success || !Number.isInteger(rowVersion)) return invalid;
  try {
    const e = await (await authedApi()).updateExperiment(experimentId, rowVersion, parsed.data);
    revalidatePath(pathsFor(orgId).research.experiments);
    return { ok: true, rowVersion: e.row_version, profileHash: e.profile_hash };
  } catch (err) {
    return failure(err);
  }
}

const promoteInput = z.object({ version: z.string().trim().min(1).max(200), notes: z.string().trim().max(2000) });

/** POST /v1/experiments/{id}/promote (`ExperimentPromote`): creates the scoring-profile draft. */
export async function promoteAction(
  orgId: string,
  experimentId: string,
  input: unknown,
): Promise<ResearchResult<{ version: string }>> {
  const parsed = promoteInput.safeParse(input);
  if (!parsed.success) return invalid;
  try {
    const doc = await (await authedApi()).promoteExperiment(experimentId, {
      kind: "scoring_profile",
      version: parsed.data.version,
      ...(parsed.data.notes ? { notes: parsed.data.notes } : {}),
    });
    revalidatePath(pathsFor(orgId).research.experiments);
    return { ok: true, version: doc.version };
  } catch (err) {
    return failure(err);
  }
}

/** POST /v1/orgs/{id}/cohorts/preview (`AnalysisRead`): counts only, suppressed below `small_cell_k` (PRV-1). */
export async function previewCohortAction(orgId: string, definition: unknown): Promise<ResearchResult<{ preview: CohortPreview }>> {
  const parsed = cohortDefinitionSchema.safeParse(definition);
  if (!parsed.success) return invalid;
  try {
    return { ok: true, preview: await (await authedApi()).previewCohort(orgId, parsed.data) };
  } catch (err) {
    return failure(err);
  }
}

const runInput = analysisRunRequestSchema.pick({ kind: true, experiment_id: true, cohort_id: true, level: true, seed: true, sweep: true, options: true });

/** POST /v1/orgs/{id}/analysis-runs (`AnalysisRun`) → 202; `429 analysis-quota-exceeded`, `400 *-too-large`. */
export async function runOnCohortAction(orgId: string, input: unknown): Promise<ResearchResult<{ runId: string }>> {
  const parsed = runInput.safeParse(input);
  if (!parsed.success || !parsed.data.cohort_id) return invalid;
  try {
    const run = await (await authedApi()).createAnalysisRun(orgId, {
      kind: parsed.data.kind,
      experiment_id: parsed.data.experiment_id,
      cohort_id: parsed.data.cohort_id,
      level: parsed.data.level ?? "auto",
      ...(parsed.data.seed != null ? { seed: parsed.data.seed } : {}),
      ...(parsed.data.kind === "param_sweep" && parsed.data.sweep ? { sweep: parsed.data.sweep, options: parsed.data.options ?? {} } : {}),
    });
    revalidatePath(base(orgId));
    return { ok: true, runId: run.id };
  } catch (err) {
    return failure(err);
  }
}

/** A stored session record for the preview: its de-identified `rawPayload` (`GET /v1/sessions/{id}/raw`). */
export interface PreviewRecord {
  id: string;
  record: Record<string, unknown>;
}

const MAX_PREVIEW_RECORDS = 10;

/** Reads the stored records to re-score in the browser. Read-only; each read is audited and org-scoped by the backend. */
export async function loadPreviewRecordsAction(ids: string[]): Promise<ResearchResult<{ records: PreviewRecord[] }>> {
  const parsed = z.array(id).max(MAX_PREVIEW_RECORDS).safeParse(ids);
  if (!parsed.success) return invalid;
  try {
    const api = await authedApi();
    const records = await Promise.all(
      parsed.data.map(async (sid) => ({ id: sid, record: (await api.getSessionRaw(sid)).raw_payload })),
    );
    return { ok: true, records };
  } catch (err) {
    return failure(err);
  }
}

/**
 * RFL-4 `GET /v1/sessions/{id}/research-frames` (`FramesRead`): the research frame log is read once per session and
 * held in memory by the preview (each read writes one `session.frames_read` audit row). On any failure the preview
 * keeps the score-level result.
 */
export async function loadResearchFramesAction(sessionId: string): Promise<ResearchResult<{ jsonl: string }>> {
  if (!id.safeParse(sessionId).success) return invalid;
  try {
    return { ok: true, jsonl: await (await authedApi()).getResearchFrames(sessionId) };
  } catch (err) {
    return failure(err);
  }
}
