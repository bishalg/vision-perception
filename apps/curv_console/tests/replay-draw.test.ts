/**
 * WP-X.20: the replay's pure parts. The draw model turns core events + frames + time into draw ops (no canvas), the
 * trace CSV is RFC 4180, and band label keys from the core resolve through the domain catalogue in both locales.
 */
import { describe, expect, it } from "vitest";
import type { ExpectedBand, GazeEvent, GazeFrame, ReplayArea, RibbonSegment } from "@/lib/asd/replay";
import {
  FIXATION_BASE_PX,
  FIXATION_SCALE_PX,
  GAZE_RADIUS_PX,
  TRAIL_SEC,
  bandRects,
  barGeometry,
  clipSamples,
  clipSpan,
  drawOps,
  fixationRadius,
  nearestPoint,
  roleToken,
  sampleIndexAt,
  segmentAt,
  stepFrame,
  tracePath,
  type ChartBox,
} from "@/components/replay/drawModel";
import { TRACE_CSV_HEADER, bandLabel, traceCsv, traceCsvName } from "@/components/replay/traceData";

function frame(t: number, x: number, y: number, valid = true, clip = "c1"): GazeFrame {
  return {
    gaze: { valid, x_norm: x, y_norm: y, video_time_sec: 100 + t, clip_media_time_sec: t },
    qualityVerdict: valid ? "optimal" : "rejected",
    stimulusId: clip,
  };
}

const frames: GazeFrame[] = [];
for (let i = 0; i < 120; i++) {
  const t = i / 60;
  frames.push(frame(t, i < 60 ? 0.25 : 0.75, 0.5, !(t >= 1.2 && t < 1.4)));
}
frames.push(frame(0, 0.9, 0.9, true, "other"));

const areas: ReplayArea[] = [
  { name: "Face", startSec: 0, endSec: 2, xMin: 0, yMin: 0, xMax: 0.5, yMax: 1, roles: ["social"] },
  { name: "Late", startSec: 1.5, endSec: 2, xMin: 0.5, yMin: 0, xMax: 1, yMax: 1, roles: ["weird"] },
];
const events: GazeEvent[] = [
  { kind: "fixation", startSec: 0, endSec: 0.95, xNorm: 0.25, yNorm: 0.5, dispersion: 0.01 },
  { kind: "saccade", startSec: 0.95, endSec: 1.0, from: { xNorm: 0.25, yNorm: 0.5 }, to: { xNorm: 0.75, yNorm: 0.5 }, amplitudeNorm: 0.5, peakVelocity: 10 },
  { kind: "fixation", startSec: 1.0, endSec: 1.19, xNorm: 0.75, yNorm: 0.5, dispersion: 0.01 },
  { kind: "blink", startSec: 1.2, endSec: 1.4 },
];

describe("samples and time", () => {
  const samples = clipSamples(frames, "c1");
  it("keeps one clip's frames in clip time order", () => {
    expect(samples).toHaveLength(120);
    expect(samples[0]).toEqual({ t: 0, x: 0.25, y: 0.5, valid: true, sessionSec: 100 });
    expect(clipSpan(samples)).toEqual({ start: 0, end: 119 / 60 });
    expect(clipSpan([])).toEqual({ start: 0, end: 0 });
  });
  it("finds the frame at a time and steps one frame either way", () => {
    expect(sampleIndexAt(samples, -1)).toBe(0);
    expect(sampleIndexAt(samples, 0.5)).toBe(30);
    expect(sampleIndexAt(samples, 99)).toBe(119);
    expect(stepFrame(samples, 0.5, 1)).toBeCloseTo(31 / 60);
    expect(stepFrame(samples, 0.5, -1)).toBeCloseTo(29 / 60);
    expect(stepFrame(samples, 0.505, -1)).toBeCloseTo(0.5);
    expect(stepFrame(samples, 0, -1)).toBe(0);
    expect(stepFrame(samples, 119 / 60, 1)).toBeCloseTo(119 / 60);
  });
  it("picks the ribbon segment and the nearest trace point", () => {
    const segs: RibbonSegment[] = [
      { startSec: 0, endSec: 1, valid: true, aoi: "Face", roles: ["social"], hits: ["Face"] },
      { startSec: 1, endSec: 2, valid: true, aoi: null, roles: [], hits: [] },
    ];
    expect(segmentAt(segs, 0.5)?.aoi).toBe("Face");
    expect(segmentAt(segs, 1)?.aoi).toBeNull();
    expect(segmentAt(segs, 3)).toBeNull();
    const pts = [0, 0.25, 0.5].map((t) => ({ tSec: t, value: t, validRatio: 1, nFrames: 60 }));
    expect(nearestPoint(pts, 0.3)?.tSec).toBe(0.25);
    expect(nearestPoint([], 0.3)).toBeNull();
  });
});

describe("draw ops", () => {
  const samples = clipSamples(frames, "c1");
  const base = { areas, events, samples, width: 100, height: 50 };

  it("fixation radius grows with duration and is capped", () => {
    expect(fixationRadius(0)).toBe(FIXATION_BASE_PX);
    expect(fixationRadius(1)).toBe(FIXATION_BASE_PX + FIXATION_SCALE_PX / 2);
    expect(fixationRadius(10)).toBe(FIXATION_BASE_PX + FIXATION_SCALE_PX);
    expect(fixationRadius(-1)).toBe(FIXATION_BASE_PX);
  });

  it("draws the active areas by role, the running fixation, the trail and the gaze point", () => {
    const ops = drawOps({ ...base, tSec: 0.5, hits: ["Face"] });
    const aois = ops.filter((o) => o.op === "aoi");
    expect(aois).toEqual([{ op: "aoi", x: 0, y: 0, w: 50, h: 50, role: "social", label: "Face", hit: true }]);
    const fix = ops.find((o) => o.op === "fixation");
    expect(fix).toMatchObject({ x: 25, y: 25, current: true, r: fixationRadius(0.5), alpha: 1 });
    const trail = ops.filter((o) => o.op === "trail");
    // 300 ms at 60 Hz: 18 or 19 samples, oldest faintest.
    expect(trail.length).toBeGreaterThanOrEqual(Math.floor(TRAIL_SEC * 60));
    expect(trail[0]!.op === "trail" && trail[0]!.alpha).toBeLessThan(0.1);
    expect(trail.at(-1)).toMatchObject({ alpha: 1, grey: false });
    expect(ops.at(-1)).toEqual({ op: "gaze", x: 25, y: 25, r: GAZE_RADIUS_PX });
    // Paint order: areas, saccades, fixations, trail, gaze.
    const order = ops.map((o) => o.op);
    expect(order.indexOf("aoi")).toBeLessThan(order.indexOf("fixation"));
    expect(order.indexOf("fixation")).toBeLessThan(order.indexOf("trail"));
  });

  it("an area outside its time and an unknown role", () => {
    expect(drawOps({ ...base, tSec: 0.5 }).some((o) => o.op === "aoi" && o.label === "Late")).toBe(false);
    const late = drawOps({ ...base, tSec: 1.6 }).find((o) => o.op === "aoi" && o.label === "Late");
    expect(late).toMatchObject({ role: "none", hit: false });
    expect(roleToken(["eye", "social"])).toBe("eye");
    expect(roleToken([])).toBe("none");
  });

  it("a saccade line fades after it ends; a finished fixation keeps its size", () => {
    const ops = drawOps({ ...base, tSec: 1.1 });
    const sac = ops.find((o) => o.op === "saccade");
    expect(sac).toMatchObject({ x1: 25, x2: 75 });
    expect(sac!.op === "saccade" && sac!.alpha).toBeLessThan(1);
    const first = ops.find((o) => o.op === "fixation" && o.x === 25);
    expect(first).toMatchObject({ current: false, r: fixationRadius(0.95) });
  });

  it("during a blink: a grey gap marker at the last valid place, no gaze point, grey trail", () => {
    const ops = drawOps({ ...base, tSec: 1.3 });
    expect(ops.some((o) => o.op === "gaze")).toBe(false);
    expect(ops.find((o) => o.op === "gap")).toEqual({ op: "gap", kind: "blink", x: 75, y: 25 });
    expect(ops.filter((o) => o.op === "trail").some((o) => o.op === "trail" && o.grey)).toBe(true);
  });

  it("without frames only areas and events", () => {
    const ops = drawOps({ ...base, samples: [], tSec: 0.5 });
    expect(ops.some((o) => o.op === "gaze" || o.op === "trail")).toBe(false);
  });
});

describe("trace chart geometry", () => {
  const box: ChartBox = { width: 110, height: 60, pad: { l: 10, r: 0, t: 10, b: 0 }, t0: 0, t1: 10 };
  const band = (over: Partial<ExpectedBand>): ExpectedBand => ({
    segmentId: "full",
    startSec: 0,
    endSec: 10,
    metricKey: "aoi_dwell_ratio::Face",
    direction: "higherBetter",
    bracketId: "24-35",
    low: 0.35,
    high: null,
    mean: 0.62,
    thresholds: [],
    citation: "",
    source: "literature",
    ...over,
  });

  it("breaks the line at an empty window", () => {
    const d = tracePath(box, [
      { tSec: 0, value: 0, validRatio: 1, nFrames: 60 },
      { tSec: 5, value: 1, validRatio: 1, nFrames: 60 },
      { tSec: 6, value: null, validRatio: 0, nFrames: 0 },
      { tSec: 10, value: 0.5, validRatio: 1, nFrames: 60 },
    ]);
    expect(d).toBe("M10.0,60.0L60.0,10.0M110.0,35.0");
  });

  it("shades an open band to the axis end and skips a band without bounds", () => {
    const [r] = bandRects(box, [band({}), band({ low: null, high: null, segmentId: "x" })]);
    expect(bandRects(box, [band({ low: null, high: null })])).toEqual([]);
    expect(r).toMatchObject({ x: 10, w: 100, y: 10, meanY: 60 - 0.62 * 50 });
    expect(r!.h).toBeCloseTo(50 * (1 - 0.35));
    const [b] = bandRects(box, [band({ direction: "band", low: 0.2, high: 0.4, startSec: 2, endSec: 4, mean: null })]);
    expect(b).toMatchObject({ x: 30, w: 20, meanY: null });
  });

  it("places the clip-card dot and band on a 0..1 track", () => {
    expect(barGeometry(0.5, band({}))).toEqual({ dot: 50, from: 35, to: 100 });
    expect(barGeometry(null, band({ low: null, high: 0.6 }))).toEqual({ dot: null, from: 0, to: 60 });
    expect(barGeometry(1.4, null)).toEqual({ dot: 100, from: 0, to: 0 });
    expect(barGeometry(null, null)).toBeNull();
  });
});

describe("trace CSV", () => {
  it("serialises the plotted points with a stable header, blanks for empty windows, CRLF", () => {
    const csv = traceCsv("clip,1", "aoiDwell:Face", [
      { tSec: 0, value: 0.5, validRatio: 1, nFrames: 60 },
      { tSec: 0.25, value: null, validRatio: 0, nFrames: 0 },
    ]);
    expect(csv).toBe(
      `${TRACE_CSV_HEADER.join(",")}\r\n"clip,1",aoiDwell:Face,0,0.5,1,60\r\n"clip,1",aoiDwell:Face,0.25,,0,0\r\n`,
    );
    expect(csv.split("\r\n")[0]).toBe("clip_id,metric,t_sec,value,valid_ratio,n_frames");
    expect(traceCsv("c", "m", [])).toBe("clip_id,metric,t_sec,value,valid_ratio,n_frames\r\n");
    expect(traceCsv('a"b', "m", [{ tSec: 1, value: 1, validRatio: 1, nFrames: 1 }])).toContain('"a""b"');
  });
  it("names the file after the clip, safely", () => {
    expect(traceCsvName("stim_01/x y")).toBe("replay-trace-stim_01_x_y.csv");
  });
});

describe("band label resolution", () => {
  it("resolves the core's label keys in both locales and shows an unknown key raw", () => {
    expect(bandLabel("en", "replay.band.expected")).toBe("Expected");
    expect(bandLabel("en", "replay.band.atypicalBelow")).toBe("Atypical below");
    expect(bandLabel("en", "replay.band.atypicalAbove")).toBe("Atypical above");
    expect(bandLabel("ne", "replay.band.expected")).toMatch(/[ऀ-ॿ]/);
    expect(bandLabel("en", "replay.band.newKind")).toBe("‹replay.band.newKind›");
  });
});
