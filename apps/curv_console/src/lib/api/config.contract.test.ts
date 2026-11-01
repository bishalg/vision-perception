/**
 * Drift guard for the baselines, scoring-profile and bundle schemas (WP-C.5), next to `catalog.contract.test.ts`: the
 * zod schemas in `config.ts` must declare every property of docs/spec/schemas/asd/api-v1.json, require the required
 * ones, accept every published example and agree with its enums. The mock's documents and bundles must parse too.
 */
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";
import type { ZodType } from "zod";
import {
  NORMATIVE_DIRECTIONS,
  ageBracketSchema,
  configBundleSummarySchema,
  flagThresholdsSchema,
  normativeBaselineSetSchema,
  normativeExpectationSchema,
  normativeRangeSchema,
  normativeSegmentSchema,
  normativeVideoSchema,
  qualityGateParamsSchema,
  scoringProfileSchema,
} from "./config";
import { createMockClient } from "./mock";

interface Def {
  enum?: string[];
  examples?: unknown[];
  required?: string[];
  properties?: Record<string, { type?: string } & Record<string, unknown>>;
}
const api = JSON.parse(
  readFileSync(resolve(process.cwd(), "../../docs/spec/schemas/asd/api-v1.json"), "utf8"),
) as { $defs: Record<string, Def> };
const defs = api.$defs;

type Shaped = ZodType & { shape: Record<string, ZodType & { isOptional(): boolean }> };

const SCHEMAS: Record<string, Shaped> = {
  NormativeBaselineSet: normativeBaselineSetSchema,
  NormativeVideo: normativeVideoSchema,
  NormativeSegment: normativeSegmentSchema,
  NormativeExpectation: normativeExpectationSchema,
  NormativeRange: normativeRangeSchema,
  AgeBracket: ageBracketSchema,
  ScoringProfile: scoringProfileSchema,
  FlagThresholds: flagThresholdsSchema,
  QualityGateParams: qualityGateParamsSchema,
  ConfigBundleSummary: configBundleSummarySchema,
} as never;

describe("api-v1.json contract: baselines, scoring profile and bundles", () => {
  for (const [name, schema] of Object.entries(SCHEMAS)) {
    const def = defs[name];
    it(`${name}: zod declares every property and requires the required ones`, () => {
      expect(def, `${name} missing from api-v1.json`).toBeDefined();
      for (const p of Object.keys(def!.properties ?? {})) {
        expect(schema.shape, `${name}.${p} not declared in zod`).toHaveProperty(p);
      }
      for (const p of def!.required ?? []) {
        expect(schema.shape[p]!.isOptional(), `${name}.${p} is required by the contract`).toBe(false);
      }
      for (const p of Object.keys(schema.shape)) {
        expect(def!.properties, `${name}.${p} is not in the contract`).toHaveProperty(p);
      }
    });
    it(`${name}: zod accepts every published example`, () => {
      for (const example of def!.examples ?? []) expect(schema.parse(example)).toBeTruthy();
    });
  }

  it("enums and integers match the contract", () => {
    expect([...NORMATIVE_DIRECTIONS]).toEqual(defs.NormativeDirection!.enum);
    expect(defs.AgeBracket!.properties!.minMonths!.type).toBe("integer");
    expect(ageBracketSchema.safeParse({ id: "x", minMonths: 1.5, maxMonths: 2 }).success).toBe(false);
  });

  it("the mock's documents and bundles parse", async () => {
    const m = createMockClient();
    const baselines = await m.listConfigDocuments("normative_baselines");
    const profiles = await m.listConfigDocuments("scoring_profile");
    expect(baselines.items.length).toBeGreaterThan(1);
    expect(profiles.items.length).toBeGreaterThan(1);
    for (const d of baselines.items) normativeBaselineSetSchema.parse(d.body);
    for (const d of profiles.items) scoringProfileSchema.parse(d.body);
    const bundles = await m.listConfigBundles();
    for (const b of bundles.items) configBundleSummarySchema.parse(b);
  });
});
