/**
 * Display helpers for the baselines and scoring-profile pages and the sandbox (WP-C.5). Nothing here decides
 * whether a document is valid or how a session scores: that is the Rust core (asd-wasm in the browser, the backend
 * on create and approve). These functions only parse for display, diff two documents for display, group the core's
 * problem lines, count the sandbox outcome and remember sandbox runs in this browser.
 */
import type { ConfigDocumentKind } from "@/lib/api/catalog";
import {
  normativeBaselineSetSchema,
  scoringProfileSchema,
  type NormativeBaselineSet,
  type ScoringProfile,
} from "@/lib/api/config";

export function parseBaselines(body: unknown): NormativeBaselineSet | null {
  const r = normativeBaselineSetSchema.safeParse(body);
  return r.success ? r.data : null;
}

export function parseProfile(body: unknown): ScoringProfile | null {
  const r = scoringProfileSchema.safeParse(body);
  return r.success ? r.data : null;
}

// ---- diff --------------------------------------------------------------------------------------------------------

export interface LeafChange {
  /** Dotted member path, e.g. `flag.greenFloor`, `videos.stim_01.segments.0.expectations.1.byAge.default.expected`. */
  path: string;
  before: unknown;
  after: unknown;
}

const isObject = (v: unknown): v is Record<string, unknown> => typeof v === "object" && v !== null && !Array.isArray(v);

function leaves(v: unknown, prefix: string, out: Map<string, unknown>): void {
  if (isObject(v)) {
    const keys = Object.keys(v);
    if (!keys.length && prefix) out.set(prefix, v);
    for (const k of keys) leaves(v[k], prefix ? `${prefix}.${k}` : k, out);
  } else if (Array.isArray(v)) {
    if (!v.length && prefix) out.set(prefix, v);
    v.forEach((x, i) => leaves(x, prefix ? `${prefix}.${i}` : String(i), out));
  } else {
    out.set(prefix, v);
  }
}

/** Leaf-by-leaf changes from `a` to `b` (absent and `null` are the same), sorted by path. For display only. */
export function diffDocuments(a: unknown, b: unknown): LeafChange[] {
  const before = new Map<string, unknown>();
  const after = new Map<string, unknown>();
  leaves(a, "", before);
  leaves(b, "", after);
  const paths = new Set([...before.keys(), ...after.keys()]);
  const out: LeafChange[] = [];
  for (const path of paths) {
    const x = before.get(path) ?? null;
    const y = after.get(path) ?? null;
    if (JSON.stringify(x) !== JSON.stringify(y)) out.push({ path, before: x, after: y });
  }
  return out.sort((p, q) => p.path.localeCompare(q.path));
}

/** Baseline changes grouped by clip id (`videos.<id>...`); other members under `""`. */
export function groupBaselineChanges(changes: LeafChange[]): Map<string, LeafChange[]> {
  const out = new Map<string, LeafChange[]>();
  for (const c of changes) {
    const m = /^videos\.([^.]+)(?:\.(.*))?$/.exec(c.path);
    const key = m ? m[1]! : "";
    const entry = m ? { ...c, path: m[2] ?? "(clip)" } : c;
    if (!out.has(key)) out.set(key, []);
    out.get(key)!.push(entry);
  }
  return out;
}

export function formatValue(v: unknown): string {
  if (v === null || v === undefined) return "—";
  if (typeof v === "string") return v;
  return JSON.stringify(v);
}

// ---- core problems ---------------------------------------------------------------------------------------------

/**
 * The Rust baseline coverage lines name their clip as a prefix (`<clip>: no segments`,
 * `<clip>/<segment>/<metric>: no range for bracket <id>`, `missing video profile: <clip>`). Group them by clip so
 * the editor can mark the clip; lines naming no clip go under `""`.
 */
export function baselineProblemsByClip(problems: string[], clipIds: string[]): Map<string, string[]> {
  const ids = new Set(clipIds);
  const out = new Map<string, string[]>();
  for (const line of problems) {
    const missing = /^missing video profile: (.+)$/.exec(line);
    const head = missing ? missing[1]! : line.split(/[:/]/, 1)[0]!;
    const key = ids.has(head) ? head : "";
    if (!out.has(key)) out.set(key, []);
    out.get(key)!.push(line);
  }
  return out;
}

// ---- sandbox outcome -----------------------------------------------------------------------------------------------

export type SandboxFlag = "green" | "yellow" | "red";

export interface SandboxOutcome {
  sessionId: string;
  /** Re-score with the published documents, or null when it failed. */
  before: { flag: SandboxFlag; composite: number | null } | null;
  /** Re-score with the draft documents, or null when it failed. */
  after: { flag: SandboxFlag; composite: number | null } | null;
}

export interface SandboxSummary {
  sessions: number;
  changed: number;
  failed: number;
  /** Mean of draft minus published composite over sessions with both; null when none. */
  meanDelta: number | null;
  /** `"green→yellow"` -> count, only changes. */
  transitions: Record<string, number>;
}

export function summarizeSandbox(outcomes: SandboxOutcome[]): SandboxSummary {
  let changed = 0;
  let failed = 0;
  let deltaSum = 0;
  let deltaN = 0;
  const transitions: Record<string, number> = {};
  for (const o of outcomes) {
    if (!o.before || !o.after) {
      failed += 1;
      continue;
    }
    if (o.before.flag !== o.after.flag) {
      changed += 1;
      const key = `${o.before.flag}→${o.after.flag}`;
      transitions[key] = (transitions[key] ?? 0) + 1;
    }
    if (o.before.composite !== null && o.after.composite !== null) {
      deltaSum += o.after.composite - o.before.composite;
      deltaN += 1;
    }
  }
  return { sessions: outcomes.length, changed, failed, meanDelta: deltaN ? deltaSum / deltaN : null, transitions };
}

// ---- sandbox runs remembered in this browser (publish gate) --------------------------------------------------------

export interface SandboxRunRecord {
  at: string;
  sessions: number;
  changed: number;
}

const runKey = (kind: ConfigDocumentKind, version: string) => `curv.sandbox.v1:${kind}:${version}`;

/** Reads the last sandbox run of a version in this browser; null when none or storage is unavailable. */
export function readSandboxRun(kind: ConfigDocumentKind, version: string): SandboxRunRecord | null {
  try {
    const raw = window.localStorage.getItem(runKey(kind, version));
    if (!raw) return null;
    const v = JSON.parse(raw) as Partial<SandboxRunRecord>;
    return typeof v.at === "string" && typeof v.sessions === "number" && typeof v.changed === "number"
      ? { at: v.at, sessions: v.sessions, changed: v.changed }
      : null;
  } catch {
    return null;
  }
}

export function recordSandboxRun(kind: ConfigDocumentKind, version: string, record: SandboxRunRecord): void {
  try {
    window.localStorage.setItem(runKey(kind, version), JSON.stringify(record));
  } catch {
    // Storage unavailable (private mode): the gate then asks again next time.
  }
}
