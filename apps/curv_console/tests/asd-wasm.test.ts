/**
 * WP-C.5: the console's Rust core wrapper (`@/lib/asd`). The envelope / panic / unavailable handling is tested with
 * fake modules; the operations run against the real asd-wasm build (skipped when it is not built), so the console's
 * result schemas are checked against what the Rust core actually returns (drift guard).
 */
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { createMockClient } from "@/lib/api/mock";
import { MOCK_BASELINES_VERSIONS, mockBaselineSet, mockScoringProfile } from "@/lib/api/mock/configSeed";
import { emptyCatalog, newArea, newClip } from "@/lib/catalog";
import type { StimulusCatalog } from "@/lib/api/catalog";
import { scoringProfileSchema } from "@/lib/api/config";
import { AsdCore, type AsdWasmExports } from "@/lib/asd/core";
import {
  contractInfo,
  defaultScoringProfile,
  rescoreApiSession,
  rescoreSession,
  validateBaselines,
  validateCatalog,
  validateScoringProfile,
} from "@/lib/asd/ops";
import { HAS_WASM, WASM_DIR, nodeCore } from "./asdCoreNode";

function emptyCatalogWithClip(): StimulusCatalog {
  const c = emptyCatalog();
  const clip = newClip([]);
  c.clips.push({ ...clip, title: "T", category: "c", areas: [{ ...newArea(clip, 0), roles: ["social"] }] });
  return c;
}

function fake(over: Partial<AsdWasmExports>): AsdWasmExports {
  const ok = () => JSON.stringify({ ok: true, result: {} });
  return {
    contractInfo: ok,
    defaultScoringProfile: ok,
    validateScoringProfile: ok,
    validateCatalog: ok,
    validateBaselines: ok,
    rescoreSession: ok,
    rescoreApiSession: ok,
    lastPanic: () => undefined,
    ...over,
  };
}

describe("AsdCore wrapper", () => {
  it("passes envelopes through and loads the module once", async () => {
    let loads = 0;
    const core = new AsdCore(async () => {
      loads += 1;
      return fake({ contractInfo: () => JSON.stringify({ ok: false, error: { kind: "invalidJson", message: "x" } }) });
    });
    expect(await core.call("validateBaselines", "{}")).toEqual({ ok: true, result: {} });
    expect(await core.call("contractInfo")).toEqual({ ok: false, error: { kind: "invalidJson", message: "x" } });
    expect(loads).toBe(1);
  });

  it("turns a wasm abort into a panic envelope and re-instantiates", async () => {
    const instances: number[] = [];
    const core = new AsdCore(async (n) => {
      instances.push(n);
      return fake({
        rescoreApiSession: () => {
          if (n === 0) throw new WebAssembly.RuntimeError("unreachable");
          return JSON.stringify({ ok: true, result: "fresh" });
        },
        lastPanic: () => "panicked at src/api.rs:1:1: boom",
      });
    });
    const first = await core.call("rescoreApiSession", "{}", "[]", "{}", "{}", "{}");
    expect(first).toEqual({ ok: false, error: { kind: "panic", message: "panicked at src/api.rs:1:1: boom" } });
    const second = await core.call("rescoreApiSession", "{}", "[]", "{}", "{}", "{}");
    expect(second).toEqual({ ok: true, result: "fresh" });
    expect(instances).toEqual([0, 1]);
  });

  it("reports a module that does not load as unavailable, and retries later", async () => {
    let attempt = 0;
    const core = new AsdCore(async () => {
      attempt += 1;
      if (attempt === 1) throw new Error("404 /asd-wasm/asd_wasm.js");
      return fake({});
    });
    const r = await core.call("contractInfo");
    expect(r.ok).toBe(false);
    if (!r.ok) expect(r.error.kind).toBe("unavailable");
    expect(await core.available()).toBe(true);
  });

  it("rejects output that is not an envelope", async () => {
    const core = new AsdCore(async () => fake({ contractInfo: () => "not json" }));
    const r = await core.call("contractInfo");
    expect(r.ok).toBe(false);
  });
});

describe.skipIf(!HAS_WASM)("real asd-wasm build", () => {
  const core = nodeCore();

  it("declares every export the console calls", () => {
    const dts = readFileSync(join(WASM_DIR!, "asd_wasm.js"), "utf8");
    for (const name of ["contractInfo", "defaultScoringProfile", "validateScoringProfile", "validateCatalog", "validateBaselines", "rescoreSession", "rescoreApiSession", "lastPanic"]) {
      expect(dts, name).toMatch(new RegExp(`export function ${name}\\(`));
    }
  });

  it("contractInfo and the default profile match the console schemas and the mock", async () => {
    const info = await contractInfo(core);
    expect(info.ok).toBe(true);
    if (info.ok) {
      expect(info.result.sessionSchemaVersions).toEqual(["1.6", "1.7", "1.8"]);
      expect(info.result.lowConfidenceValidRatio).toBe(0.6);
    }
    const profile = await defaultScoringProfile(core);
    expect(profile.ok).toBe(true);
    if (profile.ok) {
      const parsed = scoringProfileSchema.parse(profile.result);
      const { trackingQuality, ...rest } = parsed;
      expect(trackingQuality).toBeTruthy();
      // The mock's published profile is the Rust default (minus the tracking block, which decodes to the defaults).
      expect(rest).toEqual(mockScoringProfile());
    }
  });

  it("validateScoringProfile runs the backend's range checks with member paths", async () => {
    const good = await validateScoringProfile(core, mockScoringProfile());
    expect(good).toEqual({ ok: true, result: { valid: true, problems: [], fieldProblems: [] } });
    const bad = mockScoringProfile();
    bad.flag.greenFloor = 1.5;
    bad.flag.yellowFloor = 0.9;
    const r = await validateScoringProfile(core, bad);
    expect(r.ok).toBe(true);
    if (r.ok) {
      expect(r.result.valid).toBe(false);
      expect(r.result.fieldProblems.map((p) => p.path)).toEqual(["flag.greenFloor"]);
      expect(r.result.fieldProblems[0]!.message).toBe("flag.greenFloor must be in [0, 1]");
    }
    const shape = await validateScoringProfile(core, { flag: {} });
    expect(shape.ok && shape.result.valid).toBe(false);
  });

  it("validateCatalog runs the backend's catalog rules with JSON paths", async () => {
    const good = await validateCatalog(core, emptyCatalogWithClip());
    expect(good).toEqual({ ok: true, result: { valid: true, problems: [], fieldProblems: [] } });
    const bad = emptyCatalogWithClip();
    bad.clips[0]!.areas[0]!.xMin = 1.4;
    bad.fallbackLanguage = "fr";
    const r = await validateCatalog(core, bad);
    expect(r.ok).toBe(true);
    if (r.ok) {
      expect(r.result.valid).toBe(false);
      expect(r.result.fieldProblems.map((p) => p.path)).toEqual(["fallbackLanguage", "clips[0].areas[0].xMin", "clips[0].areas[0].xMax"]);
    }
    const shape = await validateCatalog(core, { clips: [] });
    expect(shape.ok && shape.result.valid).toBe(false);
  });

  it("validateBaselines accepts the mock sets and names missing coverage", async () => {
    expect(await validateBaselines(core, mockBaselineSet())).toEqual({ ok: true, result: { valid: true, problems: [], fieldProblems: [] } });
    const set = mockBaselineSet();
    set.videos.stim_02_gaze_cue_8s!.segments = [];
    const r = await validateBaselines(core, set, ["stim_99"]);
    expect(r.ok && r.result.problems).toEqual(["missing video profile: stim_99", "stim_02_gaze_cue_8s: no segments"]);
  });

  it("re-scores mock sessions: published vs strict draft changes some flags, never throws", async () => {
    const api = createMockClient();
    const page = await api.listSessions("5f0e1d2c-3b4a-4968-8776-655443322110", { limit: 20 });
    const baselines = (await api.getConfigDocument("normative_baselines", MOCK_BASELINES_VERSIONS.published)).body;
    let changed = 0;
    for (const s of page.items) {
      const [detail, clips] = await Promise.all([api.getSession(s.id), api.getSessionClips(s.id)]);
      const before = await rescoreApiSession(core, detail, clips, mockScoringProfile(), baselines, { ageMonths: 30 });
      const after = await rescoreApiSession(core, detail, clips, mockScoringProfile(true), mockBaselineSet(true), { ageMonths: 30 });
      expect(before.ok, JSON.stringify(before)).toBe(true);
      expect(after.ok).toBe(true);
      if (before.ok && after.ok) {
        expect(before.result.metricSource).toBe("sessionReport");
        expect(before.result.sessionId).toBe(s.id);
        if (detail.attention_flag === "unreliable") {
          expect(before.result.device.attentionFlag).toBeNull();
          expect(before.result.warnings.join(" ")).toMatch(/unreliable/);
        }
        if (before.result.flag.flag !== after.result.flag.flag) changed += 1;
      }
    }
    expect(changed).toBeGreaterThan(0);
  });

  it("re-scores the stored record of every mock session (1.6, 1.7 and 1.8) with the recorded clip ratios", async () => {
    const api = createMockClient();
    const page = await api.listSessions("5f0e1d2c-3b4a-4968-8776-655443322110", { limit: 64 });
    const baselines = (await api.getConfigDocument("normative_baselines", MOCK_BASELINES_VERSIONS.published)).body;
    const versions = new Set<string>();
    for (const s of page.items) {
      const raw = await api.getSessionRaw(s.id);
      versions.add(raw.schema_version);
      const r = await rescoreSession(core, raw.raw_payload, mockScoringProfile(), baselines, { ageMonths: 30 });
      expect(r.ok, JSON.stringify(r)).toBe(true);
      if (r.ok) {
        expect(r.result.sessionId).toBe(s.id);
        expect(r.result.schemaVersion).toBe(raw.schema_version);
        expect(r.result.metricSource).toBe("sessionReport");
        // The record carries the device's per-clip ratio, so no "every clip uses the session's ratio" warning.
        expect(r.result.warnings.join(" ")).not.toMatch(/no per-clip valid-sample ratio/);
      }
    }
    expect([...versions].sort()).toEqual(["1.6", "1.7", "1.8"]);
  });

  it("bad input is an envelope, not an exception", async () => {
    const api = createMockClient();
    const s = (await api.listSessions("5f0e1d2c-3b4a-4968-8776-655443322110", { limit: 1 })).items[0]!;
    const [detail, clips] = await Promise.all([api.getSession(s.id), api.getSessionClips(s.id)]);
    const r = await rescoreApiSession(core, detail, clips, mockScoringProfile(), { not: "a set" }, { ageMonths: 30 });
    expect(r.ok).toBe(false);
    if (!r.ok) expect(r.error.kind).toBe("invalidJson");
    const neg = await rescoreApiSession(core, detail, clips, mockScoringProfile(), mockBaselineSet(), { ageMonths: -1 });
    expect(!neg.ok && neg.error.kind).toBe("invalidArgument");
  });
});
