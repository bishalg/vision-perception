/**
 * Drift guard for the config-document and stimulus-catalog schemas (WP-C.4), next to `contract.test.ts`: the zod
 * schemas in `catalog.ts` must declare every property of docs/spec/schemas/asd/api-v1.json, require the required
 * ones, accept every published example and agree with its enums. The mock's documents must parse too.
 */
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";
import type { ZodType } from "zod";
import {
  CONFIG_DOCUMENT_KINDS,
  CONFIG_DOCUMENT_STATUSES,
  agePackSchema,
  areaSchema,
  catalogClipSchema,
  configDocumentPageSchema,
  configDocumentSchema,
  createConfigDocumentRequestSchema,
  membershipSchema,
  stimulusCatalogSchema,
} from "./catalog";
import { createMockClient } from "./mock";
import { SEED_CATALOG } from "./mock/catalogSeed";

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

/** Contract type name -> zod schema. */
const SCHEMAS: Record<string, Shaped> = {
  ConfigDocument: configDocumentSchema,
  ConfigDocumentPage: configDocumentPageSchema,
  CreateConfigDocumentRequest: createConfigDocumentRequestSchema,
  StimulusCatalog: stimulusCatalogSchema,
  Clip: catalogClipSchema,
  Area: areaSchema,
  Membership: membershipSchema,
  AgePack: agePackSchema,
} as never;

describe("api-v1.json contract: config documents and stimulus catalog", () => {
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

  it("integer members are integers in both", () => {
    for (const [name, key] of [["AgePack", "minMonths"], ["AgePack", "maxMonths"], ["Clip", "minMonths"], ["Clip", "maxMonths"], ["Membership", "order"]] as const) {
      expect(defs[name]!.properties![key]!.type).toBe("integer");
    }
    expect(membershipSchema.safeParse({ id: "express_30s", order: 1.5 }).success).toBe(false);
    expect(areaSchema.safeParse({ name: "A", startSec: 0.5, endSec: 1, xMin: 0.1, yMin: 0.1, xMax: 0.2, yMax: 0.2, roles: [] }).success).toBe(true);
  });

  it("enums match the contract", () => {
    expect([...CONFIG_DOCUMENT_KINDS]).toEqual(defs.ConfigDocumentKind!.enum);
    expect([...CONFIG_DOCUMENT_STATUSES]).toEqual(defs.ConfigDocumentStatus!.enum);
  });

  it("the bundled seed catalog parses, and the ConfigBundle example's catalog does too", () => {
    expect(stimulusCatalogSchema.parse(SEED_CATALOG).clips.length).toBeGreaterThan(0);
    const bundle = defs.ConfigBundle?.examples?.[0] as { catalog?: unknown } | undefined;
    if (bundle?.catalog) expect(stimulusCatalogSchema.safeParse(bundle.catalog).success).toBe(true);
  });

  it("a document from a backend-shaped response parses (seeded: no author, nulls)", () => {
    const doc = {
      id: "9b2f0c1e-1d2e-4f3a-8b4c-5d6e7f8a9b0c", kind: "stimulus_catalog", version: "bundled-1.0.0", status: "draft",
      body: SEED_CATALOG, created_by: null, created_at: "2026-10-30T09:00:00.000Z", published_at: null, notes: null,
    };
    expect(configDocumentPageSchema.parse({ items: [doc], next_cursor: null, total: null }).items[0]!.created_by).toBeNull();
    expect(configDocumentSchema.safeParse({ ...doc, status: "approved" }).success).toBe(false);
    expect(configDocumentSchema.safeParse({ ...doc, kind: "catalog" }).success).toBe(false);
  });
});

describe("mock config documents are contract-valid", () => {
  it("every listed catalog document and its body parse", async () => {
    const mock = createMockClient();
    const page = await mock.listConfigDocuments("stimulus_catalog", { limit: 100 });
    configDocumentPageSchema.parse(page);
    expect(new Set(page.items.map((d) => d.status))).toEqual(new Set(CONFIG_DOCUMENT_STATUSES));
    for (const d of page.items) expect(stimulusCatalogSchema.safeParse(d.body).success, d.version).toBe(true);
  });
});
