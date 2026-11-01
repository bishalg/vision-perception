/**
 * Editor helpers for the stimulus catalog (WP-C.4). Presentation and editing only: shape checks for form feedback,
 * battery views over the clips' memberships, AOI box geometry for dragging, and a structural diff for display.
 * Catalog rules (what makes a catalog valid) are the Rust backend's; nothing here decides them.
 */
import {
  stimulusCatalogSchema,
  type Area,
  type CatalogClip,
  type StimulusCatalog,
} from "@/lib/api/catalog";

/** Field path in the editor, e.g. `clips.2.areas.0.name`, `version`, `notes`. */
export type FieldPath = string;

export function parseCatalog(body: unknown): StimulusCatalog | null {
  const r = stimulusCatalogSchema.safeParse(body);
  return r.success ? r.data : null;
}

export interface ShapeMessages {
  required: string;
  number: string;
  integer: string;
  invalid: string;
}

function issueText(issue: { code: string; expected?: unknown; message: string }, m: ShapeMessages): string {
  if (issue.code === "invalid_type" && issue.expected === "number") return m.number;
  if (issue.code === "invalid_type" && issue.expected === "int") return m.integer;
  if (issue.code === "invalid_type") return m.required;
  return m.invalid;
}

/**
 * Shape feedback for the form: the zod mirror of the contract (types, integers, required members) plus "filled in"
 * for the identifying text fields. Returns field path -> message. The server still validates the whole body.
 */
export function shapeErrors(catalog: StimulusCatalog, version: string, m: ShapeMessages): Record<FieldPath, string> {
  const out: Record<FieldPath, string> = {};
  if (!version.trim()) out.version = m.required;
  const parsed = stimulusCatalogSchema.safeParse(catalog);
  if (!parsed.success) {
    for (const issue of parsed.error.issues) {
      const path = issue.path.map(String).join(".");
      out[path] ??= issueText(issue as { code: string; expected?: unknown; message: string }, m);
    }
  }
  const need = (path: FieldPath, value: string) => {
    if (!value.trim()) out[path] ??= m.required;
  };
  need("schemaVersion", catalog.schemaVersion);
  need("fallbackLanguage", catalog.fallbackLanguage);
  catalog.agePacks.forEach((p, i) => need(`agePacks.${i}.id`, p.id));
  catalog.clips.forEach((c, i) => {
    need(`clips.${i}.id`, c.id);
    need(`clips.${i}.title`, c.title);
    need(`clips.${i}.category`, c.category);
    c.areas.forEach((a, j) => need(`clips.${i}.areas.${j}.name`, a.name));
    c.protocols.forEach((p, j) => need(`clips.${i}.protocols.${j}.id`, p.id));
  });
  return out;
}

/** Errors under one clip (for the clip list marker). */
export function clipHasErrors(errors: Record<FieldPath, string>, index: number): boolean {
  const prefix = `clips.${index}.`;
  return Object.keys(errors).some((k) => k.startsWith(prefix));
}

/**
 * Normalise a problem+json `invalid_params[].name` to an editor path: `body.clips[2].title` and `/clips/2/title` both
 * become `clips.2.title`. A bare `body` (the backend's whole-document reasons) stays `body`.
 */
export function fieldPath(name: string): FieldPath {
  let p = name.trim().replace(/\[(\d+)\]/g, ".$1").replace(/\//g, ".").replace(/^\.+/, "");
  if (p.startsWith("body.")) p = p.slice(5);
  return p;
}

/** Split server reasons into the ones a field can show and the general ones. */
export function serverFieldErrors(params: { name: string; reason: string }[]): {
  fields: Record<FieldPath, string>;
  general: string[];
} {
  const fields: Record<FieldPath, string> = {};
  const general: string[] = [];
  for (const { name, reason } of params) {
    const path = fieldPath(name);
    if (path === "body" || path === "") general.push(reason);
    else fields[path] = fields[path] ? `${fields[path]}; ${reason}` : reason;
  }
  return { fields, general };
}

// ---- batteries -------------------------------------------------------------------------------------------------

export interface Battery {
  id: string;
  /** Clip ids in play order. */
  clipIds: string[];
}

/** Batteries are the clips' protocol memberships grouped by protocol id, each ordered by `order` (then clip order). */
export function batteriesOf(catalog: StimulusCatalog): Battery[] {
  const map = new Map<string, { clipId: string; order: number; index: number }[]>();
  catalog.clips.forEach((clip, index) => {
    for (const m of clip.protocols) {
      if (!map.has(m.id)) map.set(m.id, []);
      map.get(m.id)!.push({ clipId: clip.id, order: m.order, index });
    }
  });
  return [...map.entries()]
    .sort(([a], [b]) => a.localeCompare(b))
    .map(([id, rows]) => ({
      id,
      clipIds: rows.sort((x, y) => x.order - y.order || x.index - y.index).map((r) => r.clipId),
    }));
}

/**
 * Write a battery back into the clips: each listed clip gets membership `{id, order: position}` (0-based, as the
 * bundled catalog numbers them), every other clip loses it. Other memberships keep their place.
 */
export function setBattery(catalog: StimulusCatalog, batteryId: string, clipIds: string[]): StimulusCatalog {
  const pos = new Map(clipIds.map((id, i) => [id, i]));
  return {
    ...catalog,
    clips: catalog.clips.map((clip) => {
      const order = pos.get(clip.id);
      const at = clip.protocols.findIndex((m) => m.id === batteryId);
      if (order === undefined) {
        return at < 0 ? clip : { ...clip, protocols: clip.protocols.filter((m) => m.id !== batteryId) };
      }
      const protocols = clip.protocols.filter((m) => m.id !== batteryId);
      protocols.splice(at < 0 ? protocols.length : at, 0, { id: batteryId, order });
      return { ...clip, protocols };
    }),
  };
}

export function move<T>(list: T[], from: number, to: number): T[] {
  if (to < 0 || to >= list.length || from === to) return list;
  const next = [...list];
  const [item] = next.splice(from, 1);
  next.splice(to, 0, item!);
  return next;
}

// ---- AOI geometry --------------------------------------------------------------------------------------------

export function areasAt(clip: CatalogClip, t: number): number[] {
  return clip.areas.flatMap((a, i) => (t >= a.startSec && t <= a.endSec ? [i] : []));
}

const clamp = (v: number, lo: number, hi: number) => Math.min(hi, Math.max(lo, v));
const round3 = (v: number) => Math.round(v * 1000) / 1000;

/** Move a box by a normalised delta, keeping it inside the frame. */
export function moveBox(a: Area, dx: number, dy: number): Area {
  const w = a.xMax - a.xMin;
  const h = a.yMax - a.yMin;
  const xMin = round3(clamp(a.xMin + dx, 0, 1 - w));
  const yMin = round3(clamp(a.yMin + dy, 0, 1 - h));
  return { ...a, xMin, yMin, xMax: round3(xMin + w), yMax: round3(yMin + h) };
}

/** Drag the bottom-right corner, keeping a minimum size and the frame bounds. */
export function resizeBox(a: Area, dx: number, dy: number, min = 0.02): Area {
  return {
    ...a,
    xMax: round3(clamp(a.xMax + dx, a.xMin + min, 1)),
    yMax: round3(clamp(a.yMax + dy, a.yMin + min, 1)),
  };
}

// ---- new items -----------------------------------------------------------------------------------------------

export function emptyCatalog(): StimulusCatalog {
  return { schemaVersion: "1.0.0", languages: ["en"], fallbackLanguage: "en", agePacks: [], clips: [] };
}

export function newClip(existing: CatalogClip[]): CatalogClip {
  let n = existing.length + 1;
  const taken = new Set(existing.map((c) => c.id));
  while (taken.has(`stim_new_${n}`)) n++;
  return {
    id: `stim_new_${n}`,
    title: "",
    category: "",
    durationSec: 8,
    minMonths: 12,
    maxMonths: 60,
    protocols: [],
    areas: [],
  };
}

/** A centred box from the playhead to the end of the clip. */
export function newArea(clip: CatalogClip, t: number): Area {
  const end = Number.isFinite(clip.durationSec) && clip.durationSec > 0 ? clip.durationSec : 1;
  const start = clamp(Number.isFinite(t) ? t : 0, 0, end);
  return {
    name: `Area_${clip.areas.length + 1}`,
    startSec: round3(start),
    endSec: round3(end),
    xMin: 0.3,
    yMin: 0.3,
    xMax: 0.7,
    yMax: 0.7,
    roles: [],
  };
}

// ---- diff ------------------------------------------------------------------------------------------------------

export interface CatalogDiff {
  /** Top-level members whose value differs (`languages`, `agePacks`, ...). */
  fields: string[];
  added: string[];
  removed: string[];
  /** Clip id -> the clip members that differ. */
  changed: { id: string; fields: string[] }[];
}

const same = (a: unknown, b: unknown) => JSON.stringify(a) === JSON.stringify(b);

/** What changed from `a` to `b`, matched by clip id. For display only. */
export function diffCatalogs(a: StimulusCatalog, b: StimulusCatalog): CatalogDiff {
  const top = ["schemaVersion", "comment", "languages", "fallbackLanguage", "agePacks"] as const;
  const fields = top.filter((k) => !same(a[k] ?? null, b[k] ?? null));
  const before = new Map(a.clips.map((c) => [c.id, c]));
  const after = new Map(b.clips.map((c) => [c.id, c]));
  const added = b.clips.filter((c) => !before.has(c.id)).map((c) => c.id);
  const removed = a.clips.filter((c) => !after.has(c.id)).map((c) => c.id);
  const changed = b.clips.flatMap((c) => {
    const old = before.get(c.id);
    if (!old) return [];
    const keys = (Object.keys(c) as (keyof CatalogClip)[]).filter((k) => !same(old[k], c[k]));
    return keys.length ? [{ id: c.id, fields: keys }] : [];
  });
  return { fields: [...fields], added, removed, changed };
}
