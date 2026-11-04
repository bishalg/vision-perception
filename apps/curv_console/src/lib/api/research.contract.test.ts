/**
 * Drift guard for the researcher toolkit (WP-X.0, phase 8 step 0): the zod schemas in `research.ts` declare every
 * property of docs/spec/schemas/asd/api-v1.json and require the required ones, the enums agree, the committed
 * parameter registry (`param-registry-v1.json`) parses, every registry id has a label/help entry in both locales of
 * `messages/research.ts`, the console permissions mirror GOV-1, and the RUI-7 mock seeds parse and cover every run state.
 */
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";
import type { ZodType } from "@/lib/zod";
import { can } from "@/lib/permissions";
import { researchMessages } from "@/messages/research";
import { EXPORT_FORMATS, analysisExportSchema, isDedicatedFormat } from "./exports";
import { createMockClient, MOCK_ORG_ID } from "./mock";
import { MOCK_RESEARCH_IDS } from "./mock/research";
import {
  EXPERIMENT_STATUSES,
  LEVEL_REQUESTS,
  LEVELS,
  PARAM_STAGES,
  PARAM_UNITS,
  REPROCESS_LEVELS,
  RUN_KINDS,
  RUN_STATUSES,
  SEXES,
  analysisRunPageSchema,
  analysisRunRequestSchema,
  analysisRunSchema,
  analysisSessionPageSchema,
  analysisSessionRowSchema,
  cohortDefinitionSchema,
  cohortPageSchema,
  cohortPreviewSchema,
  cohortSchema,
  createCohortRequestSchema,
  createExperimentRequestSchema,
  distSchema,
  effectiveConfigSchema,
  experimentPageSchema,
  experimentSchema,
  isRunFinished,
  jobProgressSchema,
  paramRegistrySchema,
  paramSpecSchema,
  promoteExperimentRequestSchema,
  runSummarySchema,
  updateCohortRequestSchema,
  updateExperimentRequestSchema,
} from "./research";
import { researchFramesStateSchema } from "./types";
import { ROLES, type Role } from "./types";

interface Def {
  enum?: string[];
  oneOf?: { const?: string }[];
  required?: string[];
  properties?: Record<string, unknown>;
}
const read = (file: string) => JSON.parse(readFileSync(resolve(process.cwd(), `../../docs/spec/schemas/asd/${file}`), "utf8"));
const defs = (read("api-v1.json") as { $defs: Record<string, Def> }).$defs;
const registry = read("param-registry-v1.json") as { $id: string; registryVersion: string; params: { id: string }[] };

/** Enum values of a definition: `enum`, or the `const`s of a documented enum (`oneOf`). */
const values = (d: Def | undefined) => d?.enum ?? d?.oneOf?.map((o) => o.const);

type Shaped = ZodType & { shape: Record<string, ZodType & { isOptional(): boolean }> };

const SCHEMAS: Record<string, Shaped> = {
  ParamSpec: paramSpecSchema,
  ParamRegistry: paramRegistrySchema,
  Experiment: experimentSchema,
  ExperimentPage: experimentPageSchema,
  CreateExperimentRequest: createExperimentRequestSchema,
  UpdateExperimentRequest: updateExperimentRequestSchema,
  PromoteExperimentRequest: promoteExperimentRequestSchema,
  CohortDefinition: cohortDefinitionSchema,
  Cohort: cohortSchema,
  CohortPage: cohortPageSchema,
  CreateCohortRequest: createCohortRequestSchema,
  UpdateCohortRequest: updateCohortRequestSchema,
  CohortPreview: cohortPreviewSchema,
  AnalysisRunRequest: analysisRunRequestSchema,
  AnalysisRun: analysisRunSchema,
  AnalysisRunPage: analysisRunPageSchema,
  AnalysisSessionRow: analysisSessionRowSchema,
  AnalysisSessionPage: analysisSessionPageSchema,
  RunSummary: runSummarySchema,
  Dist: distSchema,
  JobProgress: jobProgressSchema,
  EffectiveConfig: effectiveConfigSchema,
  ResearchFramesState: researchFramesStateSchema,
  AnalysisExport: analysisExportSchema,
} as never;

/** Required members typed as arbitrary JSON (`z.unknown()` accepts `undefined`, so zod reports them optional). */
const UNKNOWN_TYPED = new Set(["ParamSpec.default"]);

describe("api-v1.json contract: researcher toolkit (WP-X.0)", () => {
  for (const [name, schema] of Object.entries(SCHEMAS)) {
    it(`${name}: zod declares every property and requires the required ones`, () => {
      const def = defs[name];
      expect(def, `${name} missing from api-v1.json`).toBeDefined();
      for (const p of Object.keys(def!.properties ?? {})) {
        expect(schema.shape, `${name}.${p} not declared in zod`).toHaveProperty(p);
      }
      for (const p of def!.required ?? []) {
        if (UNKNOWN_TYPED.has(`${name}.${p}`)) continue;
        expect(schema.shape[p]!.isOptional(), `${name}.${p} is required by the contract`).toBe(false);
      }
    });
  }

  it("enums match the contract", () => {
    expect([...PARAM_STAGES]).toEqual(values(defs.Stage));
    expect([...REPROCESS_LEVELS]).toEqual(values(defs.Reprocess));
    expect([...PARAM_UNITS]).toEqual(values(defs.Unit));
    expect([...EXPERIMENT_STATUSES]).toEqual(values(defs.ExperimentStatus));
    expect([...RUN_KINDS]).toEqual(values(defs.RunKind));
    expect([...RUN_STATUSES]).toEqual(values(defs.RunStatus));
    expect([...LEVEL_REQUESTS]).toEqual(values(defs.LevelRequest));
    expect([...LEVELS]).toEqual(values(defs.Level));
    expect([...SEXES]).toEqual(values(defs.Sex));
    expect([...EXPORT_FORMATS]).toEqual(values(defs.ExportFormat));
    expect(isDedicatedFormat("analysis_results")).toBe(true);
    expect(isRunFinished("running")).toBe(false);
    expect(isRunFinished("cancelled")).toBe(true);
  });

  it("the committed parameter registry parses and covers every message locale", () => {
    expect(registry.$id).toBe("https://schemas.curv.org/asd/param-registry-v1.json");
    const parsed = paramRegistrySchema.parse(registry);
    expect(parsed.params.length).toBeGreaterThan(80);
    const ids = parsed.params.map((p) => p.id);
    expect(new Set(ids).size).toBe(ids.length);
    for (const p of parsed.params) {
      expect(p.labelKey).toBe(`param.${p.id}.label`);
      expect(p.helpKey).toBe(`param.${p.id}.help`);
    }
    // PRM-5: both locales carry an entry for every id, in registry order (from the generated messages/domain.ts).
    for (const locale of ["en", "ne"] as const) {
      expect(Object.keys(researchMessages[locale].params), locale).toEqual(ids);
    }
  });

  it("permissions mirror GOV-1 (superadmin reads, never runs, edits, promotes or reads frames)", () => {
    const table: Record<string, Role[]> = {
      "analysis.read": ["owner", "admin", "clinician", "researcher"],
      "analysis.run": ["owner", "researcher"],
      "experiment.write": ["owner", "researcher"],
      "experiment.promote": ["owner", "researcher"],
      "frames.read": ["owner", "researcher"],
    };
    for (const [action, allowed] of Object.entries(table)) {
      for (const role of ROLES.filter((r) => r !== "superadmin")) {
        expect(can(role, action as never), `${role} ${action}`).toBe(allowed.includes(role));
      }
      expect(can("superadmin", action as never), `superadmin ${action}`).toBe(action === "analysis.read");
    }
  });

  it("mock seeds parse and cover every run state (RUI-7)", async () => {
    const api = createMockClient();
    const runs = await api.listAnalysisRuns(MOCK_ORG_ID);
    analysisRunPageSchema.parse(runs);
    const byId = new Map(runs.items.map((r) => [r.id, r]));
    expect(byId.get(MOCK_RESEARCH_IDS.runRunning)?.progress).toMatchObject({ done: 120, total: 300 });
    const done = byId.get(MOCK_RESEARCH_IDS.runSucceeded)!;
    expect(done.status).toBe("succeeded");
    expect(done.summary!.by_age_band.some((b) => b.suppressed && b.n === null)).toBe(true);
    expect(byId.get(MOCK_RESEARCH_IDS.runFailed)?.error).toBe("budget_exceeded");

    experimentPageSchema.parse(await api.listExperiments(MOCK_ORG_ID));
    cohortPageSchema.parse(await api.listCohorts(MOCK_ORG_ID));
    const preview = cohortPreviewSchema.parse(await api.previewCohort(MOCK_ORG_ID, {}));
    expect(preview.by_age_band.find((b) => b.band === "36-47")).toMatchObject({ n: null, suppressed: true });
    analysisSessionPageSchema.parse(await api.listAnalysisRunSessions(MOCK_RESEARCH_IDS.runSucceeded));
    effectiveConfigSchema.parse(await api.getEffectiveConfig(MOCK_ORG_ID));

    // The answers the pages surface.
    await expect(api.promoteExperiment(MOCK_RESEARCH_IDS.experimentPromoted, { kind: "scoring_profile", version: "x" })).rejects.toMatchObject({
      status: 409,
    });
    const e = await api.getExperiment(MOCK_RESEARCH_IDS.experimentDraft);
    await expect(api.updateExperiment(e.id, e.row_version + 1, { name: "x" })).rejects.toMatchObject({ status: 412 });
    await expect(api.createExperiment(MOCK_ORG_ID, { name: e.name })).rejects.toMatchObject({ status: 409 });
    const cancelled = await api.cancelAnalysisRun(MOCK_RESEARCH_IDS.runRunning);
    expect(cancelled.status).toBe("cancelled");
    await expect(api.cancelAnalysisRun(MOCK_RESEARCH_IDS.runRunning)).rejects.toMatchObject({ status: 409 });
  });
});
