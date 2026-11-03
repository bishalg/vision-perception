"use client";

import { useEffect, useState } from "react";
import type { SandboxSession } from "@/app/[org]/config/docActions";
import s from "@/components/catalog/catalog.module.css";
import type { CatalogResult } from "@/components/catalog/types";
import { RegistryForm } from "@/components/research/RegistryForm";
import type { SessionSummary } from "@/lib/api";
import type { NormativeBaselineSet, ScoringProfile } from "@/lib/api/config";
import { browserCore } from "@/lib/asd/browser";
import { defaultScoringProfile, validateScoringProfile } from "@/lib/asd/ops";
import { paramRegistry, type ProfileDoc } from "@/lib/research/registry";
import { configText } from "@/messages/config";
import { CoreCheckPanel, DraftShell, useCoreCheck, type DraftSubmit } from "./DraftShell";
import { SandboxRunner } from "./SandboxRunner";

const p = configText.profile;
const e = configText.editor;

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
 * Scoring profile draft editor (spec 03 §4.5, PRM-6): the tuning form is generated from the parameter registry
 * (`RegistryForm`, no per-knob fields). Every change is checked by the Rust core in the browser (asd-wasm
 * `validateScoringProfile`, the backend's own range checks) and its problems are shown at their knob.
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
  const [profile, setProfile] = useState<ProfileDoc>(initial);
  const [serverFields, setServerFields] = useState<Record<string, string>>({});
  const publishedDoc = (preview.published?.profile ?? initial) as ProfileDoc;

  const body = profile as ScoringProfile;
  const check = useCoreCheck(body, validateScoringProfile);
  const errors: Record<string, string> = { ...serverFields };
  if (check.state === "done") {
    for (const fp of check.validation.fieldProblems) errors[fp.path] = errors[fp.path] ? `${errors[fp.path]}; ${fp.message}` : fp.message;
  }
  const blocked = check.state === "done" ? check.validation.problems.length : 0;

  return (
    <DraftShell orgId={orgId} segment="scoring" fromVersion={fromVersion} body={body} blocked={blocked} save={save} onServerFields={setServerFields}>
      <CoreCheckPanel check={check} />
      <RegistryForm registry={paramRegistry} published={publishedDoc} draft={profile} context="edit" errors={errors} onChange={setProfile} />
      <details className="panel">
        <summary>{p.general}</summary>
        <div className="stack" style={{ marginTop: ".75rem" }}>
          <label>
            {p.schemaVersion}
            <input
              value={String(profile.schemaVersion ?? "")}
              onChange={(ev) => {
                const v = ev.currentTarget.value;
                setProfile((x) => ({ ...x, schemaVersion: v }));
              }}
              aria-invalid={Boolean(errors.schemaVersion) || undefined}
              className={errors.schemaVersion ? s.invalid : undefined}
            />
            {errors.schemaVersion ? <span className={s.fieldError}>{errors.schemaVersion}</span> : null}
          </label>
        </div>
      </details>
      <details className="panel">
        <summary>{e.preview}</summary>
        <p className="muted">{e.previewIntro}</p>
        {preview.published ? (
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
