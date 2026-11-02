/**
 * Drift guard for the share-a-case schemas (WP-C.19, SHR-7..9): the zod schemas in `share.ts` and the case fields in
 * `exports.ts` must declare every property of docs/spec/schemas/asd/api-v1.json and require the required ones; the
 * console's link choices must be the contract's ranges and defaults; the mock's answers must parse.
 */
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";
import type { ZodType } from "@/lib/zod";
import { caseShareRequestSchema, exportRequestSchema, exportSchema } from "./exports";
import { createMockClient, MOCK_ORG_ID } from "./mock";
import { MOCK_CASE_EXPORT_ID } from "./mock/exports";
import { MOCK_SHARE_LINKS, MOCK_SHARE_PASSCODE } from "./mock/share";
import {
  SHARE_LINK_DAY_CHOICES,
  SHARE_LINK_DEFAULT_DAYS,
  SHARE_LINK_DEFAULT_DOWNLOADS,
  SHARE_LINK_DOWNLOAD_CHOICES,
  createShareLinkRequestSchema,
  createdShareLinkSchema,
  shareLinkOpenRequestSchema,
  shareLinkOpenResponseSchema,
  shareLinkPageSchema,
  shareLinkSchema,
} from "./share";

interface Def {
  required?: string[];
  properties?: Record<string, { description?: string }>;
  examples?: unknown[];
}
const api = JSON.parse(
  readFileSync(resolve(process.cwd(), "../../docs/spec/schemas/asd/api-v1.json"), "utf8"),
) as { $defs: Record<string, Def> };
const defs = api.$defs;

type Shaped = ZodType & { shape: Record<string, ZodType & { isOptional(): boolean }> };

const SCHEMAS: Record<string, Shaped> = {
  ShareLink: shareLinkSchema,
  CreateShareLinkRequest: createShareLinkRequestSchema,
  CreatedShareLink: createdShareLinkSchema,
  ShareLinkPage: shareLinkPageSchema,
  ShareLinkOpenRequest: shareLinkOpenRequestSchema,
  ShareLinkOpenResponse: shareLinkOpenResponseSchema,
  CaseShareRequest: caseShareRequestSchema,
  Export: exportSchema,
  ExportRequest: exportRequestSchema,
} as never;

describe("api-v1.json contract: share a case (WP-C.19)", () => {
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

  it("the passcode is in the create answer only, never in a listed link", () => {
    expect(Object.keys(defs.ShareLink!.properties ?? {})).not.toContain("passcode");
    expect(defs.CreatedShareLink!.required).toContain("passcode");
    expect(Object.keys(shareLinkSchema.shape)).not.toContain("passcode");
  });

  it("the link choices are the contract's ranges and defaults (SHARE_LINK_*)", () => {
    const req = defs.CreateShareLinkRequest!.properties!;
    const days = /(\d+)\.\.(\d+); default (\d+)/.exec(req.expires_in_days!.description ?? "");
    const downloads = /(\d+)\.\.(\d+); default (\d+)/.exec(req.max_downloads!.description ?? "");
    expect(days, "expires_in_days description").not.toBeNull();
    expect(downloads, "max_downloads description").not.toBeNull();
    const [, dMin, dMax, dDef] = days!.map(Number);
    const [, nMin, nMax, nDef] = downloads!.map(Number);
    expect(Math.min(...SHARE_LINK_DAY_CHOICES)).toBe(dMin);
    expect(Math.max(...SHARE_LINK_DAY_CHOICES)).toBe(dMax);
    expect(SHARE_LINK_DEFAULT_DAYS).toBe(dDef);
    expect(SHARE_LINK_DAY_CHOICES).toContain(SHARE_LINK_DEFAULT_DAYS);
    expect([...SHARE_LINK_DOWNLOAD_CHOICES]).toEqual(Array.from({ length: nMax! - nMin! + 1 }, (_, i) => nMin! + i));
    expect(SHARE_LINK_DEFAULT_DOWNLOADS).toBe(nDef);
  });

  it("the mock returns shapes the schemas accept", async () => {
    const mock = createMockClient();
    const e = exportSchema.parse(await mock.getExport(MOCK_CASE_EXPORT_ID));
    expect([e.format, e.status]).toEqual(["case_package", "succeeded"]);
    const page = shareLinkPageSchema.parse(await mock.listShareLinks(MOCK_CASE_EXPORT_ID));
    expect(page.items.map((l) => l.id).sort()).toEqual(Object.values(MOCK_SHARE_LINKS).sort());
    const created = createdShareLinkSchema.parse(await mock.createShareLink(MOCK_CASE_EXPORT_ID, {}));
    expect(created.passcode).toMatch(/^[A-HJ-NP-Z2-9]{12}$/);
    const opened = shareLinkOpenResponseSchema.parse(await mock.openShareLink(MOCK_SHARE_LINKS.active, MOCK_SHARE_PASSCODE));
    expect(opened.export_id).toBe(MOCK_CASE_EXPORT_ID);
    const caseExport = exportSchema.parse(
      await mock.createExport(MOCK_ORG_ID, {
        format: "case_package", mode: "deidentified", purpose: "clinical_care", reason: "Second opinion for the family",
        case: { participant_id: "p1", recipient: "Dr Example, Example Children's Hospital" },
      }),
    );
    expect(caseExport.case_participant_id).toBe("p1");
    expect(caseExport.share_links_n).toBe(0);
  });
});
