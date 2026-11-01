/**
 * Typed operations over the Rust core (WP-C.5): JSON in, decoded envelope out. The zod schemas mirror the asd-wasm
 * result types (`asd-wasm/src/api.rs`: `Validation`, `RescoreResult`, `ContractInfo`) and check shape only; the
 * values are the core's. `tests/asd-wasm.test.ts` runs them against the real module.
 */
import { z } from "zod";
import type { ClipScore, SessionDetail } from "@/lib/api/types";
import type { NormativeBaselineSet, ScoringProfile } from "@/lib/api/config";
import type { AsdCore, Envelope } from "./core";

export const fieldProblemSchema = z.object({ path: z.string(), message: z.string() });
export type FieldProblem = z.infer<typeof fieldProblemSchema>;

export const validationSchema = z.object({
  valid: z.boolean(),
  problems: z.array(z.string()),
  fieldProblems: z.array(fieldProblemSchema),
});
export type Validation = z.infer<typeof validationSchema>;

export const contractInfoSchema = z.object({
  coreVersion: z.string(),
  sessionSchemaVersions: z.array(z.string()),
  sessionSchemaIds: z.array(z.string()),
  scoringProfileSchemaVersion: z.string(),
  configBundleSchemaId: z.string(),
  /** The scorer's fixed low-confidence gate; `ScoringProfile.lowConfidenceValidRatio` does not change it. */
  lowConfidenceValidRatio: z.number(),
});
export type ContractInfo = z.infer<typeof contractInfoSchema>;

const coreFlagSchema = z.enum(["green", "yellow", "red"]);
export type CoreFlag = z.infer<typeof coreFlagSchema>;

export const rescoreVideoSchema = z
  .object({
    stimulusId: z.string(),
    title: z.string(),
    rows: z.array(z.unknown()),
    subscore: z.number(),
    validSampleRatio: z.number(),
    lowConfidence: z.boolean(),
    hasBaseline: z.boolean(),
    weight: z.number(),
  })
  .loose();

export const rescoreResultSchema = z.object({
  sessionId: z.string(),
  schemaVersion: z.string(),
  metricSource: z.enum(["frames", "sessionReport"]),
  score: z
    .object({
      ageBracketId: z.string(),
      ageMonths: z.number(),
      compositeScore: z.number().nullable(),
      videos: z.array(rescoreVideoSchema),
      baselineStatus: z.string(),
      baselineSchemaVersion: z.string(),
      scoredAtUtc: z.string(),
    })
    .loose(),
  flag: z
    .object({ flag: coreFlagSchema, headline: z.string(), detail: z.string(), isScreeningSignalOnly: z.boolean() })
    .loose(),
  device: z.object({ compositeScore: z.number().nullable(), attentionFlag: coreFlagSchema.nullable() }),
  warnings: z.array(z.string()),
});
export type RescoreResult = z.infer<typeof rescoreResultSchema>;

/** Decode a successful result with `schema`; a shape mismatch is an `invalidJson` envelope (contract drift). */
function decode<T>(env: Envelope<unknown>, schema: z.ZodType<T>, what: string): Envelope<T> {
  if (!env.ok) return env;
  const r = schema.safeParse(env.result);
  return r.success
    ? { ok: true, result: r.data }
    : { ok: false, error: { kind: "invalidJson", message: `${what}: unexpected result shape (${r.error.issues[0]?.message ?? "?"})` } };
}

const json = (v: unknown) => JSON.stringify(v);

export async function contractInfo(core: AsdCore): Promise<Envelope<ContractInfo>> {
  return decode(await core.call("contractInfo"), contractInfoSchema, "contractInfo");
}

export async function defaultScoringProfile(core: AsdCore): Promise<Envelope<ScoringProfile>> {
  return (await core.call("defaultScoringProfile")) as Envelope<ScoringProfile>;
}

/** Shape plus the backend's range checks (asd-contract `profile_checks`), with the member path of each problem. */
export async function validateScoringProfile(core: AsdCore, profile: unknown): Promise<Envelope<Validation>> {
  return decode(await core.call("validateScoringProfile", json(profile)), validationSchema, "validateScoringProfile");
}

/** asd-core baseline coverage (`baselines::validate`), the check the backend runs on create and approve. */
export async function validateBaselines(
  core: AsdCore,
  set: unknown,
  requiredIds?: string[],
): Promise<Envelope<Validation>> {
  return decode(
    await core.call("validateBaselines", json(set), requiredIds ? json(requiredIds) : undefined),
    validationSchema,
    "validateBaselines",
  );
}

export interface RescoreOptions {
  ageMonths: number;
  scoredAtUtc?: string;
  validSampleRatio?: number;
}

/** Re-score a session as read from the API (detail + clip rows) with a profile and baseline set. No writes. */
export async function rescoreApiSession(
  core: AsdCore,
  session: SessionDetail,
  clips: ClipScore[],
  profile: ScoringProfile | unknown,
  baselines: NormativeBaselineSet | unknown,
  options: RescoreOptions,
): Promise<Envelope<RescoreResult>> {
  return decode(
    await core.call("rescoreApiSession", json(session), json(clips), json(profile), json(baselines), json(options)),
    rescoreResultSchema,
    "rescoreApiSession",
  );
}
