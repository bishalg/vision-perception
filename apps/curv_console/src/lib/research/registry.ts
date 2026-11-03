/**
 * The parameter registry in the console (PRM-1, PRM-6): a copy of `docs/spec/schemas/asd/param-registry-v1.json`
 * (`tests/research.test.tsx` fails when the copy drifts) read through the zod schema of `@/lib/api/research`, plus the
 * pure helpers the registry form needs: grouping, reading and writing a value at a dotted path, clamping to a row's
 * limits and the diff against the published profile. Once asd-wasm exports `paramRegistry` (WP-X.4) the form can take
 * the registry from the core instead; the shape is the same.
 *
 * Nothing here knows a parameter: every limit comes from the registry row.
 */
import { paramRegistrySchema, type ParamKind, type ParamRegistry, type ParamSpec } from "@/lib/api/research";
import registryJson from "./param-registry-v1.json";

export const paramRegistry: ParamRegistry = paramRegistrySchema.parse(registryJson);

/** A scoring profile document (camelCase), opaque: the Rust core validates it. */
export type ProfileDoc = Record<string, unknown>;

/** The group of a knob: its first path segment (a root-level knob is a group of its own, so registry order holds). */
export function groupOf(id: string): string {
  const i = id.indexOf(".");
  return i < 0 ? id : id.slice(0, i);
}

export interface ParamGroup {
  group: string;
  params: ParamSpec[];
}

/** Groups in order of first appearance; the rows keep registry order inside a group. */
export function groupRegistry(params: readonly ParamSpec[]): ParamGroup[] {
  const out: ParamGroup[] = [];
  for (const p of params) {
    const g = groupOf(p.id);
    const found = out.find((x) => x.group === g);
    if (found) found.params.push(p);
    else out.push({ group: g, params: [p] });
  }
  return out;
}

const isObject = (v: unknown): v is Record<string, unknown> => typeof v === "object" && v !== null && !Array.isArray(v);

export function getAt(doc: ProfileDoc, id: string): unknown {
  let cur: unknown = doc;
  for (const part of id.split(".")) {
    if (!isObject(cur)) return undefined;
    cur = cur[part];
  }
  return cur;
}

/** A copy of `doc` with `value` at the dotted path (missing objects are created). */
export function setAt(doc: ProfileDoc, id: string, value: unknown): ProfileDoc {
  const parts = id.split(".");
  const root: ProfileDoc = { ...doc };
  let cur = root;
  for (const part of parts.slice(0, -1)) {
    const next = cur[part];
    const copy: ProfileDoc = isObject(next) ? { ...next } : {};
    cur[part] = copy;
    cur = copy;
  }
  cur[parts[parts.length - 1]!] = value;
  return root;
}

/** The value a knob has in `doc`: the stored one, else the registry default (the Rust decoder fills the same). */
export function valueOf(doc: ProfileDoc, spec: ParamSpec): unknown {
  const v = getAt(doc, spec.id);
  return v === undefined ? spec.default : v;
}

export const sameValue = (a: unknown, b: unknown): boolean => JSON.stringify(a) === JSON.stringify(b);

/** Ids of the knobs whose effective value differs between two profiles, in registry order. */
export function diffParams(registry: readonly ParamSpec[], a: ProfileDoc, b: ProfileDoc): string[] {
  return registry.filter((p) => !sameValue(valueOf(a, p), valueOf(b, p))).map((p) => p.id);
}

const decimals = (n: number): number => {
  const s = String(n);
  if (s.includes("e-")) return Number(s.split("e-")[1]);
  const i = s.indexOf(".");
  return i < 0 ? 0 : s.length - i - 1;
};

/**
 * Clamps a typed or dragged number to a Number/Integer row: inside `[min, max]`, on the `step` grid from `min`
 * (integers: whole numbers). `null` for a non-finite input; other kinds return `null` (not numeric).
 */
export function clampParam(kind: ParamKind, raw: number): number | null {
  if (!Number.isFinite(raw)) return null;
  if (kind.type === "integer") return Math.min(kind.max, Math.max(kind.min, Math.round(raw)));
  if (kind.type !== "number") return null;
  const bounded = Math.min(kind.max, Math.max(kind.min, raw));
  const snapped = kind.min + Math.round((bounded - kind.min) / kind.step) * kind.step;
  const d = Math.max(decimals(kind.step), decimals(kind.min));
  return Math.min(kind.max, Math.max(kind.min, Number(snapped.toFixed(d))));
}

/** The slider step of a numeric row (integers step by one). */
export function stepOf(kind: ParamKind): number | undefined {
  return kind.type === "number" ? kind.step : kind.type === "integer" ? 1 : undefined;
}

export function isNumeric(kind: ParamKind): kind is Extract<ParamKind, { type: "number" | "integer" }> {
  return kind.type === "number" || kind.type === "integer";
}

/** Rows a researcher can sweep at score level: numeric knobs the score level applies. */
export function sweepableParams(registry: readonly ParamSpec[]): ParamSpec[] {
  return registry.filter((p) => isNumeric(p.kind) && p.reprocess === "score");
}

/** Short text of a value for the "default" and "published" markers. */
export function formatValue(v: unknown): string {
  if (typeof v === "number") return String(Number(v.toPrecision(6)));
  if (Array.isArray(v)) return JSON.stringify(v);
  return String(v);
}
