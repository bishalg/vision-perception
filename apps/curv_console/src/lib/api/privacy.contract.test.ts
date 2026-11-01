/**
 * Drift guard for the WP-B.9 schemas (erasure, retention sweep, dataset export), next to `contract.test.ts`: the zod
 * schemas in `privacy.ts` must declare every property of docs/spec/schemas/asd/api-v1.json, require the required
 * ones and agree with its enums; the mock's responses must parse.
 */
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";
import type { ZodType } from "zod";
import { EXPORT_FORMATS, exportRequestSchema, exportSchema } from "./exports";
import { createMockClient, MOCK_ORG_ID } from "./mock";
import {
  DATASET_FORMATS,
  ERASURE_STATUSES,
  datasetExportRequestSchema,
  erasureOutcomeSchema,
  erasureRequestPageSchema,
  erasureRequestSchema,
  retentionOrgResultSchema,
  retentionSweepJobSchema,
  retentionSweepRequestSchema,
  retentionSweepResultSchema,
} from "./privacy";

interface Def {
  enum?: string[];
  examples?: unknown[];
  required?: string[];
  properties?: Record<string, unknown>;
}
const api = JSON.parse(
  readFileSync(resolve(process.cwd(), "../../docs/spec/schemas/asd/api-v1.json"), "utf8"),
) as { $defs: Record<string, Def> };
const defs = api.$defs;

type Shaped = ZodType & { shape: Record<string, ZodType & { isOptional(): boolean }> };

const SCHEMAS: Record<string, Shaped> = {
  ErasureOutcome: erasureOutcomeSchema,
  ErasureRequest: erasureRequestSchema,
  ErasureRequestPage: erasureRequestPageSchema,
  RetentionSweepRequest: retentionSweepRequestSchema,
  RetentionSweepJob: retentionSweepJobSchema,
  RetentionOrgResult: retentionOrgResultSchema,
  RetentionSweepResult: retentionSweepResultSchema,
  DatasetExportRequest: datasetExportRequestSchema,
  // Extended by WP-B.9 (include_labels); the full check lives in contract.test.ts, repeated here for the new member.
  Export: exportSchema,
  ExportRequest: exportRequestSchema,
} as never;

describe("api-v1.json contract: erasure, retention and datasets (WP-B.9)", () => {
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
      for (const example of def!.examples ?? []) expect(schema.parse(example)).toBeTruthy();
    });
  }

  it("enums match the contract", () => {
    expect([...ERASURE_STATUSES]).toEqual(defs.ErasureStatus!.enum);
    expect([...DATASET_FORMATS]).toEqual(defs.DatasetFormat!.enum);
    expect(EXPORT_FORMATS).toContain("dataset_jsonl");
    expect(EXPORT_FORMATS).toContain("dataset_parquet");
    expect([...EXPORT_FORMATS]).toEqual(defs.ExportFormat!.enum);
  });

  it("the mock returns shapes the schemas accept", async () => {
    const mock = createMockClient();
    const page = erasureRequestPageSchema.parse(await mock.listErasureRequests(MOCK_ORG_ID));
    expect(new Set(page.items.map((r) => r.status))).toEqual(new Set(["pending", "held", "completed"]));
    const job = retentionSweepJobSchema.parse(await mock.startRetentionSweep(MOCK_ORG_ID, { dry_run: true }));
    await mock.getJob(job.job_id);
    const done = await mock.getJob(job.job_id);
    expect(retentionSweepResultSchema.parse(done.result).dry_run).toBe(true);
    const ds = exportSchema.parse(
      await mock.createDatasetExport({ format: "parquet", reason: "Dataset for the research study", include_labels: false }),
    );
    expect([ds.format, ds.mode, ds.purpose, ds.include_labels]).toEqual(["dataset_parquet", "deidentified", "research", false]);
  });
});
