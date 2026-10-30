/**
 * Drift guard (WP-C.8, API-3): the zod schemas must accept every published example in
 * docs/spec/schemas/asd/api-v1.json (generated from the Rust `asd-contract` crate), agree with its
 * enums, and declare every property it requires. A Rust-side contract change that is not mirrored in
 * types.ts fails here.
 */
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";
import type { ZodType } from "zod";
import { createMockClient, MOCK_ORG_ID } from "./mock";
import {
  ATTENTION_FLAGS,
  CAPTURE_SOURCES,
  CLIP_STATUSES,
  DIRECTIONS,
  ORG_KINDS,
  QUALITY_VERDICTS,
  ROLES,
  clipListSchema,
  clipScoreSchema,
  metricRowSchema,
  metricsSchema,
  orgSchema,
  problemSchema,
  sessionDetailSchema,
  sessionPageSchema,
  sessionSummarySchema,
  tokenPairSchema,
  userSchema,
} from "./types";

interface Def {
  enum?: string[];
  examples?: unknown[];
  required?: string[];
  properties?: Record<string, unknown>;
}
const api = JSON.parse(
  // vitest runs with cwd = apps/curv_console (npm test), so the repo root is two levels up.
  readFileSync(resolve(process.cwd(), "../../docs/spec/schemas/asd/api-v1.json"), "utf8"),
) as { $defs: Record<string, Def> };
const defs = api.$defs;

/** Contract type name -> the zod schema that validates it. */
const OBJECT_SCHEMAS: Record<string, ZodType & { shape: Record<string, ZodType & { isOptional(): boolean }> }> = {
  TokenPair: tokenPairSchema,
  User: userSchema,
  Org: orgSchema,
  Metrics: metricsSchema,
  SessionSummary: sessionSummarySchema,
  SessionDetail: sessionDetailSchema,
  ClipScore: clipScoreSchema,
  MetricRow: metricRowSchema,
  ClipList: clipListSchema,
  Problem: problemSchema,
} as never;

describe("api-v1.json contract", () => {
  it("is a generated document with $defs", () => {
    expect(Object.keys(defs).length).toBeGreaterThan(20);
  });

  for (const [name, schema] of Object.entries(OBJECT_SCHEMAS)) {
    const def = defs[name];
    it(`${name}: zod accepts every published example`, () => {
      expect(def, `${name} missing from api-v1.json`).toBeDefined();
      const examples = def!.examples ?? [];
      if (name !== "User" && name !== "Problem") expect(examples.length).toBeGreaterThan(0);
      for (const example of examples) expect(schema.parse(example)).toBeTruthy();
    });

    it(`${name}: zod declares every property and requires the required ones`, () => {
      const props = Object.keys(def!.properties ?? {});
      for (const p of props) expect(schema.shape, `${name}.${p} not declared in zod`).toHaveProperty(p);
      for (const p of def!.required ?? []) {
        expect(schema.shape[p]!.isOptional(), `${name}.${p} is required by the contract`).toBe(false);
      }
    });
  }

  it("SessionPage example shape is accepted (envelope built from SessionSummary examples)", () => {
    const items = defs.SessionSummary!.examples!;
    expect(sessionPageSchema.parse({ items, next_cursor: null, total: items.length }).items).toHaveLength(items.length);
    expect(sessionPageSchema.parse({ items: [] }).next_cursor).toBeUndefined();
  });

  it("enums match the contract", () => {
    expect([...ATTENTION_FLAGS]).toEqual(defs.SessionAttentionFlag!.enum);
    expect([...CAPTURE_SOURCES]).toEqual(defs.CaptureSource!.enum);
    expect([...CLIP_STATUSES]).toEqual(defs.ExpectationStatus!.enum);
    expect([...DIRECTIONS]).toEqual(defs.NormativeDirection!.enum);
    expect([...ORG_KINDS]).toEqual(defs.OrgKind!.enum);
    expect([...ROLES]).toEqual(defs.Role!.enum);
    expect([...QUALITY_VERDICTS]).toEqual(defs.GazeQualityVerdict!.enum);
  });

  it("rejects values outside the contract", () => {
    const ex = defs.SessionSummary!.examples![0] as Record<string, unknown>;
    expect(sessionSummarySchema.safeParse({ ...ex, attention_flag: "orange" }).success).toBe(false);
    expect(sessionSummarySchema.safeParse({ ...ex, capture_source: "mocked" }).success).toBe(false);
    const { participant_token: _omit, ...noToken } = ex;
    expect(sessionSummarySchema.safeParse(noToken).success).toBe(false);
  });

  it("battery is a free catalog protocol id (CON-16) and clip status may be null (CON-19)", () => {
    const ex = defs.SessionSummary!.examples![0] as Record<string, unknown>;
    expect(sessionSummarySchema.parse({ ...ex, battery: "comprehensive_60s" }).battery).toBe("comprehensive_60s");
    const clip = (defs.ClipScore!.examples![0] as Record<string, unknown>);
    expect(clipScoreSchema.parse({ ...clip, status: null, rows: [] }).status).toBeNull();
    expect(clipScoreSchema.parse({ clip_id: "c", order: 0 }).rows).toEqual([]);
  });
});

describe("mock client produces contract-valid data", () => {
  const api = createMockClient();

  it("every list page, detail, clip list, org, metrics and token pair parses", async () => {
    const ids: string[] = [];
    let cursor: string | undefined;
    do {
      const page = await api.listSessions(MOCK_ORG_ID, { limit: 100, cursor });
      sessionPageSchema.parse(page);
      ids.push(...page.items.map((s) => s.id));
      cursor = page.next_cursor ?? undefined;
    } while (cursor);
    expect(ids.length).toBeGreaterThan(0);
    for (const id of ids) {
      sessionDetailSchema.parse(await api.getSession(id));
      clipListSchema.parse({ items: await api.getSessionClips(id) });
    }
    orgSchema.parse(await api.getOrg(MOCK_ORG_ID));
    metricsSchema.parse(await api.getMetrics(MOCK_ORG_ID));
    tokenPairSchema.parse(await api.login("demo@example.org", "demo"));
  });

  it("covers the contract's corner cases: all flags, null clip status, empty rows, synthetic source", async () => {
    const all = (await api.listSessions(MOCK_ORG_ID, { limit: 100 })).items;
    expect(new Set(all.map((s) => s.attention_flag))).toEqual(new Set(ATTENTION_FLAGS));
    const clips = (await Promise.all(all.map((s) => api.getSessionClips(s.id)))).flat();
    expect(clips.some((c) => c.status === null)).toBe(true);
    expect(clips.some((c) => c.rows.length === 0)).toBe(true);
    expect(clips.some((c) => c.rows.length > 1)).toBe(true);
    expect(all.some((s) => s.capture_source === "synthetic")).toBe(true);
    expect(all.every((s) => QUALITY_VERDICTS.includes(s.quality_verdict as never))).toBe(true);
  });
});
