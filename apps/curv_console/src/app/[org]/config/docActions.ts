"use server";

/**
 * Server actions for the baselines and scoring-profile documents, the sandbox and the bundles (WP-C.5). The body is
 * checked for shape here; the backend runs the Rust checks again on create and approve.
 */
import { revalidatePath } from "next/cache";
import { z } from "zod";
import type { CatalogFailure, CatalogResult } from "@/components/catalog/types";
import { ApiError } from "@/lib/api";
import type { ConfigTransition } from "@/lib/api/catalog";
import {
  normativeBaselineSetSchema,
  scoringProfileSchema,
  type ConfigBundleSummary,
} from "@/lib/api/config";
import { authedApi } from "@/lib/auth/server";

/** The document kinds these pages edit, and their URL segment. */
export type EditableKind = "normative_baselines" | "scoring_profile";
const SEGMENT: Record<EditableKind, string> = { normative_baselines: "baselines", scoring_profile: "scoring" };

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

const base = (orgId: string, kind: EditableKind) => `/${encodeURIComponent(orgId)}/config/${SEGMENT[kind]}`;

const draftInput = (kind: EditableKind) =>
  z.object({
    version: z.string().max(200),
    // The changelog note is required for these kinds (spec 03 §4.5).
    notes: z.string().trim().min(1).max(2000),
    body: kind === "scoring_profile" ? scoringProfileSchema : normativeBaselineSetSchema,
  });

/** POST /v1/config/{kind} (superadmin). */
export async function createDocDraftAction(
  orgId: string,
  kind: EditableKind,
  input: unknown,
): Promise<CatalogResult<{ version: string }>> {
  if (kind !== "normative_baselines" && kind !== "scoring_profile") return { ok: false, slug: "validation-failed", status: 400 };
  const parsed = draftInput(kind).safeParse(input);
  if (!parsed.success) return { ok: false, slug: "validation-failed", status: 400 };
  const { version, notes, body } = parsed.data;
  try {
    const created = await (await authedApi()).createConfigDocument(kind, { version: version.trim(), body, notes });
    revalidatePath(base(orgId, kind));
    return { ok: true, version: created.version };
  } catch (err) {
    return failure(err);
  }
}

const STEPS: readonly ConfigTransition[] = ["approve", "publish", "retire"];

/** POST /v1/config/{kind}/{version}/approve|publish|retire (superadmin, four-eyes rule on the server). */
export async function transitionDocAction(
  orgId: string,
  kind: EditableKind,
  version: string,
  step: ConfigTransition,
): Promise<CatalogResult<{ status: string }>> {
  if (!STEPS.includes(step) || (kind !== "normative_baselines" && kind !== "scoring_profile")) {
    return { ok: false, slug: "validation-failed", status: 400 };
  }
  try {
    const doc = await (await authedApi()).transitionConfigDocument(kind, version, step);
    revalidatePath(base(orgId, kind));
    revalidatePath(`${base(orgId, kind)}/${encodeURIComponent(version)}`);
    return { ok: true, status: doc.status };
  } catch (err) {
    return failure(err);
  }
}

/** A stored session for the sandbox: its de-identified `rawPayload` record (`GET /v1/sessions/{id}/raw`). */
export interface SandboxSession {
  id: string;
  record: Record<string, unknown>;
}

const MAX_SANDBOX_SESSIONS = 50;

/** Reads the stored records to re-score. Read-only (each read is audited); the backend scopes them to the caller's org. */
export async function loadSandboxSessionsAction(
  ids: string[],
): Promise<CatalogResult<{ sessions: SandboxSession[] }>> {
  const parsed = z.array(z.string().min(1).max(200)).max(MAX_SANDBOX_SESSIONS).safeParse(ids);
  if (!parsed.success) return { ok: false, slug: "validation-failed", status: 400 };
  try {
    const api = await authedApi();
    const sessions = await Promise.all(
      parsed.data.map(async (id) => {
        const raw = await api.getSessionRaw(id);
        return { id, record: raw.raw_payload };
      }),
    );
    return { ok: true, sessions };
  } catch (err) {
    return failure(err);
  }
}

const composeInput = z.object({
  global: z.boolean(),
  featureFlags: z.record(z.string().min(1).max(100), z.boolean()),
});

/** POST /v1/config/bundles: signs the published documents (owner/admin for their org, superadmin any). */
export async function composeBundleAction(
  orgId: string,
  input: unknown,
): Promise<CatalogResult<{ bundle: ConfigBundleSummary }>> {
  const parsed = composeInput.safeParse(input);
  if (!parsed.success) return { ok: false, slug: "validation-failed", status: 400 };
  try {
    const bundle = await (await authedApi()).composeConfigBundle({
      ...(parsed.data.global ? {} : { org_id: orgId }),
      ...(Object.keys(parsed.data.featureFlags).length ? { feature_flags: parsed.data.featureFlags } : {}),
    });
    revalidatePath(`/${encodeURIComponent(orgId)}/config/bundles`);
    return { ok: true, bundle };
  } catch (err) {
    return failure(err);
  }
}
