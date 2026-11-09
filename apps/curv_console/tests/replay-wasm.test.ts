/**
 * WP-X.20: the replay wrappers (`@/lib/asd/replay`) against the real asd-wasm build (skipped when it is not built), over
 * the mock session frames and the mock effective configuration. Guards the zod mirrors of the Rust replay types and
 * the mock fixture the e2e tests and screenshots rely on (two clips, a blink, a loss gap, bands for the face AOI).
 */
import { describe, expect, it } from "vitest";
import { createMockClient } from "@/lib/api/mock";
import { MOCK_REPLAY_FACE } from "@/lib/api/mock/replay";
import {
  ageMonthsOfRecord,
  catalogClip,
  framesOfRecord,
  heatmapCohort,
  heatmapDifference,
  replayEvents,
  replayHeatmap,
  replayRibbon,
  replayTrace,
  expectedBands,
  targetAoiOf,
} from "@/lib/asd/replay";
import { HAS_WASM, nodeCore } from "./asdCoreNode";

const ORG = "5f0e1d2c-3b4a-4968-8776-655443322110";

async function fixture() {
  const api = createMockClient();
  const page = await api.listSessions(ORG, { limit: 1 });
  const id = page.items[0]!.id;
  const raw = await api.getSessionRaw(id);
  const config = await api.getEffectiveConfig(ORG);
  return { raw, config };
}

describe("mock replay fixture", () => {
  it("carries deterministic frames, an age and the catalog clips", async () => {
    const a = await fixture();
    const b = await fixture();
    const frames = framesOfRecord(a.raw.raw_payload)!;
    expect(frames).not.toBeNull();
    expect(frames).toEqual(framesOfRecord(b.raw.raw_payload));
    expect(new Set(frames.map((f) => f.stimulusId)).size).toBe(2);
    expect(ageMonthsOfRecord(a.raw.raw_payload)).toBeGreaterThan(0);
    const clipId = frames[0]!.stimulusId!;
    const clip = catalogClip(a.config.stimulus_catalog.body, clipId);
    expect(clip?.areas.length).toBeGreaterThan(0);
    expect(targetAoiOf(a.config.normative_baselines.body, clip)).toBe(MOCK_REPLAY_FACE);
    expect(framesOfRecord({})).toBeNull();
    expect(ageMonthsOfRecord({ ageMonths: -1 })).toBeNull();
  });
});

describe.skipIf(!HAS_WASM)("replay wrappers against the Rust core", () => {
  it("events, ribbon, trace with bands, heatmaps", async () => {
    const core = nodeCore();
    const { raw, config } = await fixture();
    const frames = framesOfRecord(raw.raw_payload)!;
    const profile = config.scoring_profile.body;
    const ev = await replayEvents(core, frames, profile);
    expect(ev.ok, JSON.stringify(ev)).toBe(true);
    if (!ev.ok) return;
    expect(ev.result.clips).toHaveLength(2);
    const kinds = new Set(ev.result.clips.flatMap((c) => c.events.map((e) => e.kind)));
    expect([...kinds].sort()).toEqual(["blink", "fixation", "lost", "saccade"]);

    const clip = catalogClip(config.stimulus_catalog.body, ev.result.clips[0]!.stimulusId)!;
    const ribbon = await replayRibbon(core, frames, clip, profile);
    expect(ribbon.ok, JSON.stringify(ribbon)).toBe(true);
    if (!ribbon.ok) return;
    expect(ribbon.result.segments.some((s) => s.aoi === MOCK_REPLAY_FACE)).toBe(true);

    const trace = await replayTrace(core, frames, clip, config.normative_baselines.body, profile, {
      metric: { kind: "aoiDwell", aoi: MOCK_REPLAY_FACE },
      windowSec: 1,
      stepSec: 0.25,
      ageMonths: 30,
    });
    expect(trace.ok, JSON.stringify(trace)).toBe(true);
    if (!trace.ok) return;
    expect(trace.result.trace.points.length).toBeGreaterThan(10);
    expect(trace.result.bands.length).toBe(1);
    expect(trace.result.bands[0]!.thresholds.map((t) => t.labelKey)).toContain("replay.band.expected");
    const values = trace.result.trace.points.map((p) => p.value).filter((v): v is number => v !== null);
    const mean = values.reduce((a, b) => a + b, 0) / values.length;
    expect(mean).toBeGreaterThan(0.45);
    expect(mean).toBeLessThan(0.75);

    const bands = await expectedBands(core, config.normative_baselines.body, clip.id, 30, "aoi_dwell_ratio", MOCK_REPLAY_FACE);
    expect(bands.ok && bands.result.length).toBe(1);

    const grid = await replayHeatmap(core, frames, { clipId: clip.id, w: 64, h: 36, bandwidthNorm: 0.04 });
    expect(grid.ok, JSON.stringify(grid)).toBe(true);
    if (!grid.ok) return;
    expect(grid.result.cells).toHaveLength(64 * 36);
    expect(Math.max(...grid.result.cells)).toBeCloseTo(1, 5);

    const suppressed = await heatmapCohort(core, [grid.result]);
    expect(suppressed.ok && suppressed.result.suppressed).toBe(true);
    const cohort = await heatmapCohort(core, [grid.result, grid.result], 2);
    expect(cohort.ok && cohort.result.grid?.w).toBe(64);
    const diff = await heatmapDifference(core, grid.result, grid.result);
    expect(diff.ok && Math.max(...diff.result.cells.map(Math.abs))).toBe(0);
  });

  it("a draft profile with I-DT changes the fixations", async () => {
    const core = nodeCore();
    const { raw, config } = await fixture();
    const frames = framesOfRecord(raw.raw_payload)!;
    const base = config.scoring_profile.body as Record<string, unknown>;
    const idt = { ...base, metrics: { ...(base.metrics as object), fixationAlgorithm: "idt", idtDispersionNorm: 0.01 } };
    const a = await replayEvents(core, frames, base);
    const b = await replayEvents(core, frames, idt);
    expect(a.ok && b.ok).toBe(true);
    if (!a.ok || !b.ok) return;
    expect(b.result.params.algorithm).toBe("idt");
    expect(JSON.stringify(a.result.clips)).not.toEqual(JSON.stringify(b.result.clips));
  });
});
