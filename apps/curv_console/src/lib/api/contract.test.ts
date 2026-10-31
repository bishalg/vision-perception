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
  PROBLEM_SLUGS,
  QUALITY_VERDICTS,
  ROLES,
  CALIBRATION_QUALITIES,
  CAL_MODEL_KINDS,
  GEOMETRY_SOURCES,
  ORIENTATIONS,
  RECAL_REASONS,
  TRACKING_VERDICTS,
  sessionCalibrationSchema,
  sessionDriftCheckSchema,
  sessionRecalibrationSchema,
  sessionTrackingConditionsSchema,
  sessionTrackingConfidenceSchema,
  sessionTrackingDeviceSchema,
  sessionTrackingLossSchema,
  sessionTrackingQualitySchema,
  sessionValidationSchema,
  sessionValidationTargetSchema,
  REVIEW_REASONS,
  REVIEW_STATUSES,
  USER_STATUSES,
  clipListSchema,
  clipScoreSchema,
  deviceSchema,
  devicePageSchema,
  enrolCodeRequestSchema,
  enrolCodeResponseSchema,
  labelInputSchema,
  metricRowSchema,
  metricsSchema,
  orgSchema,
  problemSchema,
  reviewItemSchema,
  reviewPageSchema,
  sessionDetailSchema,
  sessionPageSchema,
  sessionSummarySchema,
  tokenPairSchema,
  updateReviewRequestSchema,
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
  SessionTrackingQuality: sessionTrackingQualitySchema,
  SessionCalibration: sessionCalibrationSchema,
  SessionValidation: sessionValidationSchema,
  SessionValidationTarget: sessionValidationTargetSchema,
  SessionDriftCheck: sessionDriftCheckSchema,
  SessionRecalibration: sessionRecalibrationSchema,
  SessionTrackingConditions: sessionTrackingConditionsSchema,
  SessionTrackingConfidence: sessionTrackingConfidenceSchema,
  SessionTrackingDevice: sessionTrackingDeviceSchema,
  SessionTrackingLoss: sessionTrackingLossSchema,
  ClipScore: clipScoreSchema,
  MetricRow: metricRowSchema,
  ClipList: clipListSchema,
  Problem: problemSchema,
  ReviewItem: reviewItemSchema,
  ReviewPage: reviewPageSchema,
  UpdateReviewRequest: updateReviewRequestSchema,
  LabelInput: labelInputSchema,
  Device: deviceSchema,
  DevicePage: devicePageSchema,
  EnrolCodeRequest: enrolCodeRequestSchema,
  EnrolCodeResponse: enrolCodeResponseSchema,
} as never;

/**
 * Types the contract publishes without `examples` (request bodies, envelopes, and the review/device items
 * that WP-B.3 has not given examples yet). Their properties and required members are still checked, and the
 * fixtures below stand in for examples.
 */
const NO_PUBLISHED_EXAMPLES = new Set([
  "User", "Problem", "ReviewItem", "ReviewPage", "UpdateReviewRequest", "LabelInput", "Device", "DevicePage",
  "EnrolCodeRequest", "SessionTrackingQuality", "SessionCalibration", "SessionValidation",
  "SessionValidationTarget", "SessionDriftCheck", "SessionRecalibration", "SessionTrackingConditions",
  "SessionTrackingConfidence", "SessionTrackingDevice", "SessionTrackingLoss",
]);

describe("api-v1.json contract", () => {
  it("is a generated document with $defs", () => {
    expect(Object.keys(defs).length).toBeGreaterThan(20);
  });

  for (const [name, schema] of Object.entries(OBJECT_SCHEMAS)) {
    const def = defs[name];
    it(`${name}: zod accepts every published example`, () => {
      expect(def, `${name} missing from api-v1.json`).toBeDefined();
      const examples = def!.examples ?? [];
      if (!NO_PUBLISHED_EXAMPLES.has(name)) expect(examples.length).toBeGreaterThan(0);
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
    expect([...USER_STATUSES]).toEqual(defs.UserStatus!.enum);
    expect([...PROBLEM_SLUGS]).toEqual(defs.ProblemSlug!.enum);
    expect([...REVIEW_REASONS]).toEqual(defs.ReviewReason!.enum);
    expect([...REVIEW_STATUSES]).toEqual(defs.ReviewStatus!.enum);
    expect([...TRACKING_VERDICTS]).toEqual(defs.TrackingVerdict!.enum);
    expect([...CALIBRATION_QUALITIES]).toEqual(defs.CalibrationQuality!.enum);
    expect([...CAL_MODEL_KINDS]).toEqual(defs.CalModelKind!.enum);
    expect([...RECAL_REASONS]).toEqual(defs.RecalReason!.enum);
    expect([...GEOMETRY_SOURCES]).toEqual(defs.GeometrySource!.enum);
    expect([...ORIENTATIONS]).toEqual(defs.Orientation!.enum);
  });

  it("SessionDetail is accepted with and without tracking_quality (1.6 sessions carry null)", () => {
    const examples = defs.SessionDetail!.examples as Record<string, unknown>[];
    expect(examples.some((e) => e.tracking_quality)).toBe(true);
    expect(examples.some((e) => e.tracking_quality === null)).toBe(true);
    for (const e of examples) {
      const parsed = sessionDetailSchema.parse(e);
      expect(parsed.tracking_quality ?? null).toEqual(e.tracking_quality ?? null);
    }
    const without = { ...examples.find((e) => e.tracking_quality)! };
    delete without.tracking_quality;
    expect(sessionDetailSchema.parse(without).tracking_quality).toBeUndefined();
    const bad = { ...examples.find((e) => e.tracking_quality)!, tracking_quality: { verdict: "bogus" } };
    expect(sessionDetailSchema.safeParse(bad).success).toBe(false);
  });

  it("review and device shapes the backend emits are accepted; out-of-contract values are not", () => {
    const item = {
      id: "6a1b2c3d-4e5f-4a6b-8c7d-9e0f1a2b3c4d", org_id: "o", session_id: "S1", reason: "flag_red",
      status: "open", created_at: "2026-10-30T09:00:00.000Z", assigned_to: null, decision: null,
      decided_by: null, decided_at: null,
    };
    expect(reviewPageSchema.parse({ items: [item], next_cursor: null, total: 1 }).items[0]!.reason).toBe("flag_red");
    expect(reviewItemSchema.safeParse({ ...item, reason: "quality_low" }).success).toBe(false);
    expect(reviewItemSchema.safeParse({ ...item, status: "closed" }).success).toBe(false);
    const device = {
      id: "d", org_id: "o", platform: "ios", app_version: "1.4.0", enrolled_at: "2026-10-30T09:00:00.000Z",
      label: null, revoked_at: null, last_seen_at: null, config_bundle_version: null,
    };
    expect(devicePageSchema.parse({ items: [device], next_cursor: null, total: null }).items).toHaveLength(1);
    expect(updateReviewRequestSchema.parse({ status: "done", labels: [{ key: "k", value: true }] }).labels).toHaveLength(1);
    expect(labelInputSchema.safeParse({ key: "k" }).success).toBe(false);
  });

  it("User.org_id is null for the superadmin (no org) and a string otherwise", () => {
    const ex = defs.User!.examples![0] as Record<string, unknown>;
    expect(userSchema.parse({ ...ex, role: "superadmin", org_id: null }).org_id).toBeNull();
    expect(userSchema.parse(ex).org_id).toBe(ex.org_id);
    expect(userSchema.safeParse({ ...ex, org_id: 7 }).success).toBe(false);
  });

  it("rejects values outside the contract", () => {
    const ex = defs.SessionSummary!.examples![0] as Record<string, unknown>;
    expect(sessionSummarySchema.safeParse({ ...ex, attention_flag: "orange" }).success).toBe(false);
    expect(sessionSummarySchema.safeParse({ ...ex, capture_source: "mocked" }).success).toBe(false);
    const noToken = { ...ex };
    delete noToken.participant_token;
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
    reviewPageSchema.parse(await api.listReview(MOCK_ORG_ID, { limit: 100 }));
    devicePageSchema.parse(await api.listDevices(MOCK_ORG_ID));
    enrolCodeResponseSchema.parse(await api.createEnrolCode(MOCK_ORG_ID, {}));
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
