/**
 * Mock-mode replay fixtures (WP-X.20): deterministic synthetic `GazeFrame`s for every mock session, the catalog clips
 * they play (with areas of interest) and the published expectations their traces are read against. Synthetic test
 * data, not a recording and not a norm: two clips at 60 Hz, a fixation pattern that rests on the face area about 60 %
 * of the time, one blink and one tracking-loss gap, so e2e tests and screenshots are stable.
 */
import { MOCK_SESSION_CLIPS } from "./configSeed";

export const MOCK_REPLAY_HZ = 60;

type Area = { name: string; startSec: number; endSec: number; xMin: number; yMin: number; xMax: number; yMax: number; roles: string[] };

/** The face area every replay clip has; the published expectation reads its dwell (`aoi_dwell_ratio::Actor_Face`). */
export const MOCK_REPLAY_FACE = "Actor_Face";

const CLIP_A = MOCK_SESSION_CLIPS[0];
const CLIP_B = MOCK_SESSION_CLIPS[1];
const DUR_A = 10;
const DUR_B = 8;

/** Catalog clips (`StimulusCatalog.clips[]` shape) of the two replay clips. */
export const MOCK_REPLAY_CLIPS = [
  {
    id: CLIP_A,
    title: "Social versus geometric split screen",
    category: "Social Engagement",
    durationSec: DUR_A,
    minMonths: 12,
    maxMonths: 60,
    protocols: [{ id: "express_30s", order: 1 }, { id: "comprehensive_60s", order: 1 }],
    areas: [
      { name: MOCK_REPLAY_FACE, startSec: 0, endSec: DUR_A, xMin: 0.08, yMin: 0.15, xMax: 0.42, yMax: 0.75, roles: ["social"] },
      { name: "Geometric_Pattern", startSec: 0, endSec: DUR_A, xMin: 0.58, yMin: 0.15, xMax: 0.92, yMax: 0.75, roles: ["geometric"] },
    ] satisfies Area[],
  },
  {
    id: CLIP_B,
    title: "Gaze cue to a toy",
    category: "Joint Attention",
    durationSec: DUR_B,
    minMonths: 12,
    maxMonths: 60,
    protocols: [{ id: "express_30s", order: 2 }, { id: "comprehensive_60s", order: 2 }],
    areas: [
      { name: MOCK_REPLAY_FACE, startSec: 0, endSec: DUR_B, xMin: 0.3, yMin: 0.08, xMax: 0.7, yMax: 0.5, roles: ["social", "eye"] },
      { name: "Toy_Left", startSec: 2, endSec: DUR_B, xMin: 0.04, yMin: 0.6, xMax: 0.26, yMax: 0.92, roles: ["orienting"] },
      { name: "Toy_Right", startSec: 2, endSec: DUR_B, xMin: 0.74, yMin: 0.6, xMax: 0.96, yMax: 0.92, roles: [] },
    ] satisfies Area[],
  },
];

/** `NormativeBaselineSet.videos` entries of the replay clips (dwell on the face area; synthetic values). */
export function mockReplayBaselineVideos(): Record<string, unknown> {
  const expectation = (direction: string, byAge: Record<string, Record<string, number>>) => ({
    biomarker: "aoi_dwell_ratio",
    targetAOI: MOCK_REPLAY_FACE,
    direction,
    byAge,
    citation: "Synthetic mock value (face dwell).",
    source: "literature",
  });
  return {
    [CLIP_A]: {
      title: MOCK_REPLAY_CLIPS[0]!.title, category: "Social", weight: 0.5,
      segments: [{
        id: "full", description: "Whole clip", startSec: 0, endSec: DUR_A,
        expectations: [expectation("higherBetter", {
          default: { expected: 0.6, acceptableLow: 0.35 },
          "24-35": { expected: 0.62, acceptableLow: 0.35 },
        })],
      }],
    },
    [CLIP_B]: {
      title: MOCK_REPLAY_CLIPS[1]!.title, category: "Attention", weight: 0.5,
      segments: [
        {
          id: "face", description: "Before the cue", startSec: 0, endSec: 2,
          expectations: [expectation("band", { default: { expected: 0.6, acceptableLow: 0.4, acceptableHigh: 0.8 } })],
        },
        {
          id: "cue", description: "After the cue", startSec: 2, endSec: DUR_B,
          expectations: [expectation("band", { default: { expected: 0.45, acceptableLow: 0.25, acceptableHigh: 0.65 } })],
        },
      ],
    },
  };
}

function hash(s: string): number {
  let h = 2166136261;
  for (let i = 0; i < s.length; i++) h = Math.imul(h ^ s.charCodeAt(i), 16777619);
  return h >>> 0;
}

function rng(seed: number): () => number {
  let s = seed >>> 0;
  return () => {
    s = (s + 0x6d2b79f5) >>> 0;
    let t = s;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

/** The child's age in months the mock record states (deterministic per session). */
export function mockAgeMonths(sessionId: string): number {
  return 24 + (hash(sessionId) % 12);
}

const center = (a: Area) => [(a.xMin + a.xMax) / 2, (a.yMin + a.yMax) / 2] as const;
const r3 = (v: number) => Math.round(v * 1000) / 1000;

/**
 * Synthetic frames of one session: per second of clip time 0.6 s on the face, 0.4 s elsewhere (a short saccade in
 * between), sub-pixel jitter; clip A has a 0.15 s blink at 4 s, clip B a 0.8 s tracking-loss gap at 3 s.
 */
export function mockReplayFrames(sessionId: string): Record<string, unknown>[] {
  const r = rng(hash(sessionId));
  const frames: Record<string, unknown>[] = [];
  let sessionSec = 0;
  const step = 1 / MOCK_REPLAY_HZ;
  for (const clip of MOCK_REPLAY_CLIPS) {
    const face = clip.areas[0]!;
    const others = clip.areas.slice(1);
    const n = Math.round(clip.durationSec * MOCK_REPLAY_HZ);
    const offset = r() * 0.1;
    const pick = Math.floor(r() * 7);
    for (let i = 0; i < n; i++) {
      const t = i * step;
      const phase = (t + offset) % 1;
      const second = Math.floor(t + offset);
      const away = others[(second + pick) % others.length] ?? face;
      let x: number;
      let y: number;
      if (phase < 0.55) [x, y] = center(face);
      else if (phase < 0.6) {
        // saccade face -> away
        const k = (phase - 0.55) / 0.05;
        const [fx, fy] = center(face);
        const [ax, ay] = center(away);
        x = fx + (ax - fx) * k;
        y = fy + (ay - fy) * k;
      } else if (phase < 0.95) [x, y] = center(away);
      else {
        const k = (phase - 0.95) / 0.05;
        const [fx, fy] = center(face);
        const [ax, ay] = center(away);
        x = ax + (fx - ax) * k;
        y = ay + (fy - ay) * k;
      }
      x = r3(x + (r() - 0.5) * 0.006);
      y = r3(y + (r() - 0.5) * 0.006);
      const blink = clip.id === CLIP_A && t >= 4 && t < 4.15;
      const lost = clip.id === CLIP_B && t >= 3 && t < 3.8;
      const valid = !blink && !lost;
      const hits = valid
        ? clip.areas.filter((a) => t >= a.startSec && t < a.endSec && x >= a.xMin && x <= a.xMax && y >= a.yMin && y <= a.yMax).map((a) => a.name)
        : [];
      frames.push({
        gaze: {
          valid,
          x_norm: valid ? x : 0,
          y_norm: valid ? y : 0,
          video_time_sec: r3(sessionSec + t),
          clip_media_time_sec: r3(t),
        },
        affect: {},
        activeAOINames: hits,
        qualityVerdict: valid ? "optimal" : "rejected",
        stimulusId: clip.id,
      });
    }
    sessionSec += clip.durationSec;
  }
  return frames;
}
