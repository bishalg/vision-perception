"use client";

import { useRouter } from "next/navigation";
import { useState, useTransition, type ReactNode } from "react";
import { AOI_ROLES, type Area, type CatalogClip, type StimulusCatalog } from "@/lib/api/catalog";
import {
  clipHasErrors,
  fieldPath,
  newArea,
  newClip,
  serverFieldErrors,
  shapeErrors,
  type FieldPath,
} from "@/lib/catalog";
import { useCoreCheck } from "@/components/config/DraftShell";
import { validateCatalog } from "@/lib/asd/ops";
import { AoiFrame } from "./AoiFrame";
import { AoiTimeline } from "./AoiTimeline";
import { BatteryPanel } from "./BatteryPanel";
import s from "./catalog.module.css";
import { catalogFailureText, type CatalogResult } from "./types";
import { useText } from "@/lib/i18n/provider";


export interface DraftInput {
  version: string;
  notes?: string;
  body: StimulusCatalog;
}

const errId = (path: FieldPath) => `err-${path.replace(/[^A-Za-z0-9_-]/g, "-")}`;

function FieldError({ path, errors }: { path: FieldPath; errors: Record<FieldPath, string> }) {
  const msg = errors[path];
  return msg ? <span id={errId(path)} className={s.fieldError}>{msg}</span> : null;
}

function TextField({
  label,
  path,
  value,
  onChange,
  errors,
  hint,
}: {
  label: string;
  path: FieldPath;
  value: string;
  onChange: (v: string) => void;
  errors: Record<FieldPath, string>;
  hint?: ReactNode;
}) {
  const bad = Boolean(errors[path]);
  return (
    <label>
      {label}
      <input
        value={value}
        onChange={(ev) => onChange(ev.currentTarget.value)}
        aria-invalid={bad || undefined}
        aria-describedby={bad ? errId(path) : undefined}
        className={bad ? s.invalid : undefined}
      />
      {hint ? <span className="muted">{hint}</span> : null}
      <FieldError path={path} errors={errors} />
    </label>
  );
}

function NumField({
  label,
  path,
  value,
  onChange,
  errors,
  step = 1,
  min,
  max,
}: {
  label: string;
  path: FieldPath;
  value: number;
  onChange: (v: number) => void;
  errors: Record<FieldPath, string>;
  step?: number;
  min?: number;
  max?: number;
}) {
  const bad = Boolean(errors[path]);
  return (
    <label>
      {label}
      <input
        type="number"
        step={step}
        min={min}
        max={max}
        value={Number.isFinite(value) ? value : ""}
        onChange={(ev) => onChange(ev.currentTarget.value === "" ? NaN : ev.currentTarget.valueAsNumber)}
        aria-invalid={bad || undefined}
        aria-describedby={bad ? errId(path) : undefined}
        className={bad ? s.invalid : undefined}
      />
      <FieldError path={path} errors={errors} />
    </label>
  );
}

/**
 * Compose a new catalog draft: catalog settings, clips with their AOI timelines and boxes, and batteries. Shape
 * problems are shown before sending; everything else is decided by the backend, whose problem+json reasons are shown
 * at the field they name (or above the form when they name the whole body).
 */
export function CatalogEditor({
  orgId,
  initial,
  fromVersion,
  save,
}: {
  orgId: string;
  initial: StimulusCatalog;
  fromVersion: string | null;
  save: (input: DraftInput) => Promise<CatalogResult<{ version: string }>>;
}) {
  const tx = useText();
  const t = tx.catalog;
  const e = tx.catalog.editor;
  const router = useRouter();
  const [catalog, setCatalog] = useState<StimulusCatalog>(initial);
  const [version, setVersion] = useState("");
  const [notes, setNotes] = useState("");
  const [languages, setLanguages] = useState(initial.languages.join(", "));
  const [clipIdx, setClipIdx] = useState<number | null>(initial.clips.length ? 0 : null);
  const [areaIdx, setAreaIdx] = useState<number | null>(null);
  const [time, setTime] = useState(0);
  const [attempted, setAttempted] = useState(false);
  const [server, setServer] = useState<{ message: string; fields: Record<FieldPath, string>; general: string[] } | null>(null);
  const [pending, startTransition] = useTransition();

  const local = attempted ? shapeErrors(catalog, version, t.shape) : {};
  // The Rust core's own catalog rules (asd-wasm `validateCatalog`, the backend's checks) run on every change; each
  // problem is shown at the field its JSON path names. When the module cannot load, the server still decides.
  const check = useCoreCheck(catalog, validateCatalog);
  const core: Record<FieldPath, string> = {};
  const coreGeneral: string[] = [];
  if (check.state === "done") {
    for (const fp of check.validation.fieldProblems) {
      const path = fieldPath(fp.path);
      core[path] = core[path] ? `${core[path]}; ${fp.message}` : fp.message;
    }
    if (check.validation.fieldProblems.length === 0) coreGeneral.push(...check.validation.problems);
  }
  const errors: Record<FieldPath, string> = { ...(server?.fields ?? {}), ...core, ...local };
  const localCount = Object.keys({ ...core, ...local }).length + coreGeneral.length;

  const update = (fn: (c: StimulusCatalog) => StimulusCatalog) => setCatalog((c) => fn(c));
  const updateClip = (i: number, patch: Partial<CatalogClip>) =>
    update((c) => ({ ...c, clips: c.clips.map((x, k) => (k === i ? { ...x, ...patch } : x)) }));
  const updateArea = (ci: number, ai: number, patch: Partial<Area>) =>
    update((c) => ({
      ...c,
      clips: c.clips.map((x, k) => (k === ci ? { ...x, areas: x.areas.map((a, j) => (j === ai ? { ...a, ...patch } : a)) } : x)),
    }));

  const clip = clipIdx !== null ? catalog.clips[clipIdx] : undefined;
  const area = clip && areaIdx !== null ? clip.areas[areaIdx] : undefined;
  const cp = `clips.${clipIdx}`;
  const ap = `${cp}.areas.${areaIdx}`;

  const selectClip = (i: number | null) => {
    setClipIdx(i);
    setAreaIdx(null);
    setTime(0);
  };

  function submit() {
    setAttempted(true);
    const found = shapeErrors(catalog, version, t.shape);
    if (Object.keys(found).length || Object.keys(core).length || coreGeneral.length) return;
    startTransition(async () => {
      const r = await save({ version: version.trim(), ...(notes.trim() ? { notes: notes.trim() } : {}), body: catalog });
      if (r.ok) {
        router.push(`/${encodeURIComponent(orgId)}/config/catalog/${encodeURIComponent(r.version)}?created=1`);
        return;
      }
      const { fields, general } = serverFieldErrors(r.params ?? []);
      // 409 config-version-exists names no field; it is about the version.
      if (r.slug === "config-version-exists") fields.version = catalogFailureText(r, tx.catalog.errors);
      setServer({ message: catalogFailureText(r, tx.catalog.errors), fields, general });
    });
  }

  const serverList = server
    ? [...server.general, ...Object.entries(server.fields).map(([k, v]) => `${k}: ${v}`)]
    : [];

  return (
    <div className="stack">
      <p className="muted">{fromVersion ? e.from(fromVersion) : e.blank}</p>

      <section className="panel stack" aria-label={e.title}>
        <div className={s.grid}>
          <TextField label={e.version} path="version" value={version} onChange={setVersion} errors={errors} hint={e.versionHint} />
        </div>
        <label>
          {e.notes}
          <textarea rows={2} value={notes} maxLength={2000} onChange={(ev) => setNotes(ev.currentTarget.value)} />
          <FieldError path="notes" errors={errors} />
        </label>
      </section>

      <details className={`panel ${s.section}`}>
        <summary>{e.settings}</summary>
        <div className={s.grid} style={{ marginTop: ".75rem" }}>
          <TextField label={e.schemaVersion} path="schemaVersion" value={catalog.schemaVersion} onChange={(v) => update((c) => ({ ...c, schemaVersion: v }))} errors={errors} />
          <label>
            {e.languages}
            <input
              value={languages}
              onChange={(ev) => {
                const raw = ev.currentTarget.value;
                setLanguages(raw);
                update((c) => ({ ...c, languages: raw.split(",").map((x) => x.trim()).filter(Boolean) }));
              }}
            />
            <FieldError path="languages" errors={errors} />
          </label>
          <TextField label={e.fallbackLanguage} path="fallbackLanguage" value={catalog.fallbackLanguage} onChange={(v) => update((c) => ({ ...c, fallbackLanguage: v }))} errors={errors} />
        </div>
        <label style={{ marginTop: ".75rem" }}>
          {e.comment}
          <textarea
            rows={3}
            value={catalog.comment ?? ""}
            onChange={(ev) => {
              const v = ev.currentTarget.value;
              update((c) => {
                const next = { ...c };
                if (v) next.comment = v;
                else delete next.comment;
                return next;
              });
            }}
          />
        </label>
        <h3>{e.agePacks}</h3>
        {catalog.agePacks.map((p, i) => (
          <div key={i} className={s.row}>
            <TextField label={e.agePackId} path={`agePacks.${i}.id`} value={p.id} errors={errors}
              onChange={(v) => update((c) => ({ ...c, agePacks: c.agePacks.map((x, k) => (k === i ? { ...x, id: v } : x)) }))} />
            <NumField label={t.clips.minMonths} path={`agePacks.${i}.minMonths`} value={p.minMonths} errors={errors}
              onChange={(v) => update((c) => ({ ...c, agePacks: c.agePacks.map((x, k) => (k === i ? { ...x, minMonths: v } : x)) }))} />
            <NumField label={t.clips.maxMonths} path={`agePacks.${i}.maxMonths`} value={p.maxMonths} errors={errors}
              onChange={(v) => update((c) => ({ ...c, agePacks: c.agePacks.map((x, k) => (k === i ? { ...x, maxMonths: v } : x)) }))} />
            <button type="button" className={s.small} onClick={() => update((c) => ({ ...c, agePacks: c.agePacks.filter((_, k) => k !== i) }))}>
              {e.removeAgePack}
            </button>
          </div>
        ))}
        <button type="button" className={s.small} style={{ marginTop: ".5rem" }}
          onClick={() => update((c) => ({ ...c, agePacks: [...c.agePacks, { id: "", minMonths: 0, maxMonths: 0 }] }))}>
          {e.addAgePack}
        </button>
      </details>

      <h2 className={s.section}>{t.clips.title}</h2>
      <div className={s.layout}>
        <nav aria-label={t.clips.list} className="stack">
          {catalog.clips.length === 0 ? <p className="muted">{t.clips.none}</p> : null}
          <ul className={s.clipList}>
            {catalog.clips.map((c, i) => (
              <li key={i}>
                <button type="button" className={s.clipButton} aria-current={clipIdx === i} onClick={() => selectClip(i)}>
                  <span>{c.title || c.id || "?"}</span>
                  <span className={s.clipMeta}>
                    {c.id} · {t.clips.seconds(c.durationSec)} · {t.clips.areas(c.areas.length)}
                  </span>
                  {clipHasErrors(errors, i) ? <span className={s.errorMark}>{t.clips.hasErrors}</span> : null}
                </button>
              </li>
            ))}
          </ul>
          <button type="button" className={s.small}
            onClick={() => {
              update((c) => ({ ...c, clips: [...c.clips, newClip(c.clips)] }));
              selectClip(catalog.clips.length);
            }}>
            {t.clips.add}
          </button>
        </nav>

        {clip && clipIdx !== null ? (
          <section className="panel stack" aria-label={`${t.clips.title}: ${clip.id}`}>
            <div className={s.grid}>
              <TextField label={t.clips.id} path={`${cp}.id`} value={clip.id} errors={errors} onChange={(v) => updateClip(clipIdx, { id: v })} />
              <TextField label={t.clips.clipTitle} path={`${cp}.title`} value={clip.title} errors={errors} onChange={(v) => updateClip(clipIdx, { title: v })} />
              <TextField label={t.clips.category} path={`${cp}.category`} value={clip.category} errors={errors} onChange={(v) => updateClip(clipIdx, { category: v })} />
              <NumField label={t.clips.duration} path={`${cp}.durationSec`} value={clip.durationSec} step={0.1} min={0} errors={errors} onChange={(v) => updateClip(clipIdx, { durationSec: v })} />
              <NumField label={t.clips.minMonths} path={`${cp}.minMonths`} value={clip.minMonths} min={0} errors={errors} onChange={(v) => updateClip(clipIdx, { minMonths: v })} />
              <NumField label={t.clips.maxMonths} path={`${cp}.maxMonths`} value={clip.maxMonths} min={0} errors={errors} onChange={(v) => updateClip(clipIdx, { maxMonths: v })} />
            </div>
            <p className="muted" style={{ margin: 0 }}>
              {t.clips.batteries}:{" "}
              {clip.protocols.length ? clip.protocols.map((m) => `${m.id} #${m.order + 1}`).join(", ") : t.clips.noBattery}
            </p>
            <div className={s.row}>
              <button type="button" className={s.small}
                onClick={() => {
                  update((c) => ({ ...c, clips: [...c.clips, { ...structuredClone(clip), id: `${clip.id}_copy`, protocols: [] }] }));
                  selectClip(catalog.clips.length);
                }}>
                {t.clips.duplicate}
              </button>
              <button type="button" className={`danger ${s.small}`}
                onClick={() => {
                  update((c) => ({ ...c, clips: c.clips.filter((_, k) => k !== clipIdx) }));
                  selectClip(catalog.clips.length > 1 ? Math.max(0, clipIdx - 1) : null);
                }}>
                {t.clips.remove}
              </button>
            </div>

            <h3>{t.aoi.title}</h3>
            <AoiTimeline clip={clip} selected={areaIdx} onSelect={setAreaIdx} time={time} onTime={setTime} />
            <AoiFrame clip={clip} time={time} selected={areaIdx} onSelect={setAreaIdx}
              onChange={(ai, a) => updateArea(clipIdx, ai, a)} />
            <div className={s.row}>
              <button type="button" className={s.small}
                onClick={() => {
                  updateClip(clipIdx, { areas: [...clip.areas, newArea(clip, time)] });
                  setAreaIdx(clip.areas.length);
                }}>
                {t.aoi.add}
              </button>
            </div>

            {area && areaIdx !== null ? (
              <fieldset className="panel stack" aria-label={`${t.aoi.selected}: ${area.name}`}>
                <div className={s.grid}>
                  <TextField label={t.aoi.name} path={`${ap}.name`} value={area.name} errors={errors} onChange={(v) => updateArea(clipIdx, areaIdx, { name: v })} />
                  <NumField label={t.aoi.start} path={`${ap}.startSec`} value={area.startSec} step={0.1} min={0} errors={errors} onChange={(v) => updateArea(clipIdx, areaIdx, { startSec: v })} />
                  <NumField label={t.aoi.end} path={`${ap}.endSec`} value={area.endSec} step={0.1} min={0} errors={errors} onChange={(v) => updateArea(clipIdx, areaIdx, { endSec: v })} />
                </div>
                <div className={s.grid}>
                  {(["xMin", "yMin", "xMax", "yMax"] as const).map((k) => (
                    <NumField key={k} label={t.aoi[k]} path={`${ap}.${k}`} value={area[k]} step={0.01} min={0} max={1} errors={errors}
                      onChange={(v) => updateArea(clipIdx, areaIdx, { [k]: v })} />
                  ))}
                </div>
                <fieldset className={s.roles}>
                  <legend>{t.aoi.roles}</legend>
                  {AOI_ROLES.map((r) => (
                    <label key={r}>
                      <input
                        type="checkbox"
                        checked={area.roles.includes(r)}
                        onChange={(ev) => {
                          const on = ev.currentTarget.checked;
                          updateArea(clipIdx, areaIdx, { roles: on ? [...area.roles, r] : area.roles.filter((x) => x !== r) });
                        }}
                      />
                      {t.aoi.roleNames[r]}
                    </label>
                  ))}
                </fieldset>
                {area.roles.some((r) => !(AOI_ROLES as readonly string[]).includes(r)) ? (
                  <p className="muted">{t.aoi.otherRoles}: {area.roles.filter((r) => !(AOI_ROLES as readonly string[]).includes(r)).join(", ")}</p>
                ) : null}
                {area.roles.length === 0 ? <p className="muted">{t.aoi.noRole}</p> : null}
                <FieldError path={`${ap}.roles`} errors={errors} />
                <div>
                  <button type="button" className={`danger ${s.small}`}
                    onClick={() => {
                      updateClip(clipIdx, { areas: clip.areas.filter((_, j) => j !== areaIdx) });
                      setAreaIdx(null);
                    }}>
                    {t.aoi.remove}
                  </button>
                </div>
              </fieldset>
            ) : null}
          </section>
        ) : null}
      </div>

      <h2 className={s.section}>{t.batteries.title}</h2>
      <BatteryPanel catalog={catalog} onChange={setCatalog} />

      <section className={`panel ${s.section}`}>
        {attempted && localCount ? (
          <div role="alert" className={s.alert}>
            <p style={{ margin: 0 }}>{e.fixFirst(localCount)}</p>
            {coreGeneral.length ? <ul>{coreGeneral.map((m, i) => <li key={i}>{m}</li>)}</ul> : null}
          </div>
        ) : null}
        {server ? (
          <div role="alert" className={s.alert}>
            <strong>{server.message}</strong>
            {serverList.length ? (
              <>
                <p style={{ margin: ".25rem 0 0" }}>{e.serverRejected}</p>
                <ul>{serverList.map((m, i) => <li key={i}>{m}</li>)}</ul>
              </>
            ) : null}
          </div>
        ) : null}
        <div className={s.row}>
          <button type="button" onClick={submit} disabled={pending}>{pending ? e.saving : e.submit}</button>
          <a href={`/${encodeURIComponent(orgId)}/config/catalog`}>{e.cancel}</a>
        </div>
      </section>
    </div>
  );
}
