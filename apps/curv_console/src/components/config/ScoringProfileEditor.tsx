"use client";

import { useEffect, useState } from "react";
import type { SandboxSession } from "@/app/[org]/config/docActions";
import s from "@/components/catalog/catalog.module.css";
import type { CatalogResult } from "@/components/catalog/types";
import type { SessionSummary } from "@/lib/api";
import type { NormativeBaselineSet, ScoringProfile } from "@/lib/api/config";
import { browserCore } from "@/lib/asd/browser";
import { defaultScoringProfile, validateScoringProfile } from "@/lib/asd/ops";
import { configText } from "@/messages/config";
import { CoreCheckPanel, DraftShell, useContractInfo, useCoreCheck, type DraftSubmit } from "./DraftShell";
import { PROFILE_FIELDS } from "./DocViews";
import { SandboxRunner } from "./SandboxRunner";
import c from "./config.module.css";

const p = configText.profile;
const e = configText.editor;

const errId = (path: string) => `err-${path.replace(/[^A-Za-z0-9_-]/g, "-")}`;

function NumberField({
  label,
  path,
  value,
  onChange,
  errors,
  step,
  hint,
}: {
  label: string;
  path: string;
  value: number;
  onChange: (v: number) => void;
  errors: Record<string, string>;
  step: number;
  hint?: string;
}) {
  const msg = errors[path];
  return (
    <label>
      {label}
      <input
        type="number"
        step={step}
        value={Number.isFinite(value) ? value : ""}
        onChange={(ev) => onChange(ev.currentTarget.value === "" ? NaN : ev.currentTarget.valueAsNumber)}
        aria-invalid={Boolean(msg) || undefined}
        aria-describedby={msg ? errId(path) : undefined}
        className={msg ? s.invalid : undefined}
      />
      {hint ? <span className="muted">{hint}</span> : null}
      {msg ? <span id={errId(path)} className={s.fieldError}>{msg}</span> : null}
    </label>
  );
}

type EditorProps = Parameters<typeof ScoringProfileEditor>[0];

/**
 * The editor, starting from `initial` or, when there is no source version, from the Rust core's default profile
 * (asd-wasm `defaultScoringProfile`).
 */
export function ScoringProfileDraft(props: Omit<EditorProps, "initial"> & { initial: ScoringProfile | null }) {
  const [loaded, setLoaded] = useState<ScoringProfile | null | "unavailable">(props.initial);
  useEffect(() => {
    if (props.initial) return;
    let live = true;
    void defaultScoringProfile(browserCore()).then((r) => {
      if (live) setLoaded(r.ok ? r.result : "unavailable");
    });
    return () => {
      live = false;
    };
  }, [props.initial]);
  if (loaded === "unavailable") return <p className={s.alert} role="alert">{configText.sandbox.coreUnavailable}</p>;
  if (!loaded) return <p className="muted" role="status">{e.checking}</p>;
  return <ScoringProfileEditor {...props} initial={loaded} />;
}

/**
 * Scoring profile draft editor (spec 03 §4.5): flag thresholds, the low-confidence ratio and quality-gate parameters
 * as fields, tracking-quality parameters as JSON. Every change is checked by the Rust core in the browser
 * (asd-wasm `validateScoringProfile`, the backend's own range checks) and its problems are shown at their field.
 * "Preview on recent sessions" re-scores recent sessions with the unsaved draft against the published version.
 */
export function ScoringProfileEditor({
  orgId,
  initial,
  fromVersion,
  preview,
  save,
}: {
  orgId: string;
  initial: ScoringProfile;
  fromVersion: string | null;
  preview: {
    sessions: SessionSummary[];
    published: { label: string; profile: unknown; baselines: NormativeBaselineSet } | null;
    load: (ids: string[]) => Promise<CatalogResult<{ sessions: SandboxSession[] }>>;
  };
  save: (input: DraftSubmit<ScoringProfile>) => Promise<CatalogResult<{ version: string }>>;
}) {
  const { info } = useContractInfo();
  const fixedLowConfidence = info ? String(info.lowConfidenceValidRatio) : "…";
  const [profile, setProfile] = useState<ScoringProfile>(initial);
  const [tracking, setTracking] = useState(initial.trackingQuality ? JSON.stringify(initial.trackingQuality, null, 2) : "");
  const [serverFields, setServerFields] = useState<Record<string, string>>({});

  let trackingValue: Record<string, unknown> | undefined;
  let trackingError: string | undefined;
  if (tracking.trim()) {
    try {
      const parsed: unknown = JSON.parse(tracking);
      if (parsed && typeof parsed === "object" && !Array.isArray(parsed)) trackingValue = parsed as Record<string, unknown>;
      else trackingError = p.trackingInvalid;
    } catch {
      trackingError = p.trackingInvalid;
    }
  }
  const numbersOk = [...PROFILE_FIELDS.flag.map((k) => profile.flag[k]), ...PROFILE_FIELDS.qualityGate.map((k) => profile.qualityGate[k]), profile.lowConfidenceValidRatio].every(Number.isFinite);
  const body: ScoringProfile | undefined =
    trackingError || !numbersOk
      ? undefined
      : (() => {
          const { trackingQuality: _drop, ...rest } = profile;
          void _drop;
          return trackingValue ? { ...rest, trackingQuality: trackingValue } : rest;
        })();

  const check = useCoreCheck(body, validateScoringProfile);
  const errors: Record<string, string> = { ...serverFields };
  if (check.state === "done") {
    for (const fp of check.validation.fieldProblems) errors[fp.path] = errors[fp.path] ? `${errors[fp.path]}; ${fp.message}` : fp.message;
  }
  // Non-finite inputs never reach the core (JSON has no NaN): flag them here.
  for (const k of PROFILE_FIELDS.flag) if (!Number.isFinite(profile.flag[k])) errors[`flag.${k}`] = e.number;
  for (const k of PROFILE_FIELDS.qualityGate) if (!Number.isFinite(profile.qualityGate[k])) errors[`qualityGate.${k}`] = e.number;
  if (!Number.isFinite(profile.lowConfidenceValidRatio)) errors.lowConfidenceValidRatio = e.number;
  const blocked = check.state === "done" ? check.validation.problems.length : 0;

  const setFlag = (k: (typeof PROFILE_FIELDS.flag)[number], v: number) => setProfile((x) => ({ ...x, flag: { ...x.flag, [k]: v } }));
  const setGate = (k: (typeof PROFILE_FIELDS.qualityGate)[number], v: number) =>
    setProfile((x) => ({ ...x, qualityGate: { ...x.qualityGate, [k]: v } }));

  return (
    <DraftShell orgId={orgId} segment="scoring" fromVersion={fromVersion} body={body} blocked={blocked} save={save} onServerFields={setServerFields}>
      <CoreCheckPanel check={check} />
      <div className={c.columns}>
        <section className="panel stack" aria-label={p.flag}>
          <h2 style={{ margin: 0 }}>{p.flag}</h2>
          {PROFILE_FIELDS.flag.map((k) => (
            <NumberField key={k} label={p[k]} path={`flag.${k}`} value={profile.flag[k]} step={0.01} errors={errors} onChange={(v) => setFlag(k, v)} />
          ))}
          <NumberField
            label={p.lowConfidenceValidRatio}
            path="lowConfidenceValidRatio"
            value={profile.lowConfidenceValidRatio}
            step={0.01}
            errors={errors}
            hint={p.lowConfidenceCaveat(fixedLowConfidence)}
            onChange={(v) => setProfile((x) => ({ ...x, lowConfidenceValidRatio: v }))}
          />
        </section>
        <section className="panel stack" aria-label={p.qualityGate}>
          <h2 style={{ margin: 0 }}>{p.qualityGate}</h2>
          {PROFILE_FIELDS.qualityGate.map((k) => (
            <NumberField key={k} label={p[k]} path={`qualityGate.${k}`} value={profile.qualityGate[k]} step={k.endsWith("Cm") ? 1 : 0.01} errors={errors} onChange={(v) => setGate(k, v)} />
          ))}
        </section>
      </div>
      <details className="panel">
        <summary>{p.general}</summary>
        <div className="stack" style={{ marginTop: ".75rem" }}>
          <label>
            {p.schemaVersion}
            <input value={profile.schemaVersion} onChange={(ev) => {
              const v = ev.currentTarget.value;
              setProfile((x) => ({ ...x, schemaVersion: v }));
            }} aria-invalid={Boolean(errors.schemaVersion) || undefined} className={errors.schemaVersion ? s.invalid : undefined} />
            {errors.schemaVersion ? <span className={s.fieldError}>{errors.schemaVersion}</span> : null}
          </label>
          <label>
            {p.tracking}
            <textarea rows={8} value={tracking} onChange={(ev) => setTracking(ev.currentTarget.value)}
              aria-invalid={Boolean(trackingError) || undefined} className={trackingError ? s.invalid : undefined} spellCheck={false} />
            <span className="muted">{p.trackingHint}</span>
            {trackingError ? <span className={s.fieldError}>{trackingError}</span> : null}
          </label>
        </div>
      </details>
      <details className="panel">
        <summary>{e.preview}</summary>
        <p className="muted">{e.previewIntro}</p>
        {body && preview.published ? (
          <SandboxRunner
            sessions={preview.sessions}
            initialSelected={preview.sessions.map((x) => x.id)}
            published={preview.published}
            draft={{ label: e.preview, profile: body, baselines: preview.published.baselines }}
            load={preview.load}
          />
        ) : (
          <p className="muted">{configText.sandbox.noBaselines}</p>
        )}
      </details>
    </DraftShell>
  );
}
