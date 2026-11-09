"use client";

import Link from "next/link";
import { useState, useTransition } from "react";
import type { PreviewRecord } from "@/app/[org]/research/actions";
import { DraftChecks } from "./DraftChecks";
import type { Cohort, CohortPreview, Dataset, Experiment, ParamRegistry } from "@/lib/api/research";
import { useText } from "@/lib/i18n/provider";
import { paramRegistry as bundledRegistry, sameValue, type ProfileDoc } from "@/lib/research/registry";
import { failureText, fieldReasons, type ResearchResult } from "@/lib/research/result";
import type { ResearchMessages } from "@/messages/research";
import { ExperimentHeader } from "./ExperimentHeader";
import { PreviewPanel } from "./PreviewPanel";
import { RegistryForm } from "./RegistryForm";
import { RunDialog, type RunStartInput } from "./RunDialog";
import type { PickerSession } from "./SessionPicker";
import r from "./research.module.css";
import { pathsFor } from "@/lib/paths";

export interface WorkbenchActions {
  update: (rowVersion: number, input: { name: string; description: string | null; profile: ProfileDoc }) => Promise<ResearchResult<{ rowVersion: number; profileHash: string }>>;
  promote: (input: { version: string; notes: string }) => Promise<ResearchResult<{ version: string }>>;
  previewCohort: (definition: Cohort["definition"]) => Promise<ResearchResult<{ preview: CohortPreview }>>;
  runOnCohort: (input: RunStartInput) => Promise<ResearchResult<{ runId: string }>>;
  loadRecords: (ids: string[]) => Promise<ResearchResult<{ records: PreviewRecord[] }>>;
  loadFrames: (id: string) => Promise<ResearchResult<{ jsonl: string }>>;
}

/**
 * The experiment workbench (RUI-1..5): the registry form on the left edits the draft profile; the preview on the right
 * re-scores the picked sessions with the published profile and the draft as you move a knob. Saving sends the draft with
 * `If-Match`; the Rust core's problems show at their knob and block saving, the server's `field_problems` are shown the
 * same way.
 */
export function Workbench({
  orgId,
  experiment,
  registry = bundledRegistry,
  publishedProfile,
  baselines,
  catalog,
  sessions,
  cohorts,
  datasets = [],
  initialDatasetId,
  canEdit,
  canRun,
  canPromote,
  actions,
  text: textProp,
}: {
  orgId: string;
  experiment: Experiment;
  registry?: ParamRegistry;
  publishedProfile: ProfileDoc | null;
  baselines: unknown | null;
  catalog?: unknown;
  sessions: PickerSession[];
  cohorts: Cohort[];
  /** WP-X.11: frozen datasets for the run dialog; `initialDatasetId` opens it on one. */
  datasets?: Pick<Dataset, "id" | "name" | "member_count" | "content_hash" | "split">[];
  initialDatasetId?: string;
  canEdit: boolean;
  canRun: boolean;
  canPromote: boolean;
  actions: WorkbenchActions;
  text?: ResearchMessages;
}) {
  const bundle = useText();
  const text = textProp ?? bundle.research;
  const w = text.workbench;
  const [name, setName] = useState(experiment.name);
  const [description, setDescription] = useState(experiment.description ?? "");
  const [draft, setDraft] = useState<ProfileDoc>(experiment.profile);
  const [saved, setSaved] = useState({ name: experiment.name, description: experiment.description ?? "", profile: experiment.profile, rowVersion: experiment.row_version });
  const lastRunId = experiment.last_run_id ?? null;
  const [status, setStatus] = useState(experiment.status);
  const [promotedVersion, setPromotedVersion] = useState<string | null>(null);
  const [message, setMessage] = useState<{ kind: "ok" | "error"; text: string; list?: string[] } | null>(null);
  const [serverFields, setServerFields] = useState<Record<string, string>>({});
  const [coreProblems, setCoreProblems] = useState<{ count: number; fields: Record<string, string> }>({ count: 0, fields: {} });
  const [promoteError, setPromoteError] = useState<string | null>(null);
  const [running, setRunning] = useState(Boolean(initialDatasetId) && canRun);
  const [pending, start] = useTransition();

  const dirty = name !== saved.name || description !== saved.description || !sameValue(draft, saved.profile);
  const errors = { ...serverFields, ...coreProblems.fields };
  const editable = canEdit && status === "draft";

  function save() {
    setMessage(null);
    setServerFields({});
    start(async () => {
      const res = await actions.update(saved.rowVersion, { name: name.trim(), description: description.trim() || null, profile: draft });
      if (res.ok) {
        setSaved({ name: name.trim(), description: description.trim(), profile: draft, rowVersion: res.rowVersion });
        setMessage({ kind: "ok", text: w.saved });
        return;
      }
      const fields = fieldReasons(res.params);
      setServerFields(fields);
      setMessage({ kind: "error", text: failureText(res, text), list: Object.entries(fields).map(([k, v]) => `${k}: ${v}`) });
    });
  }

  function promote(input: { version: string; notes: string }) {
    setPromoteError(null);
    start(async () => {
      const res = await actions.promote(input);
      if (res.ok) {
        setPromotedVersion(res.version);
        setStatus("promoted");
      } else setPromoteError(failureText(res, text));
    });
  }

  return (
    <div className="stack">
      <p><Link href={pathsFor(orgId).research.experiments}>{w.back}</Link></p>
      <ExperimentHeader
        experiment={{ ...experiment, status, last_run_id: lastRunId }}
        name={name}
        description={description}
        onName={setName}
        onDescription={setDescription}
        readOnly={!editable}
        saving={pending}
        canSave={editable && dirty && name.trim().length > 0 && coreProblems.count === 0}
        onSave={save}
        canRun={canRun && status !== "archived"}
        onRun={() => setRunning(true)}
        canPromote={canPromote && status === "draft" && Boolean(lastRunId) && !dirty}
        onPromote={promote}
        promoteError={promoteError}
        text={text}
      />
      {promotedVersion ? <p role="status">{w.promoted(promotedVersion)}</p> : null}
      {message ? (
        <div role={message.kind === "error" ? "alert" : "status"}>
          {message.text}
          {message.list?.length ? <><br />{w.serverRejected}<ul>{message.list.map((x) => <li key={x}>{x}</li>)}</ul></> : null}
        </div>
      ) : null}
      {running ? (
        <RunDialog
          orgId={orgId}
          experimentId={experiment.id}
          cohorts={cohorts}
          datasets={datasets}
          {...(initialDatasetId ? { initialDatasetId } : {})}
          dirty={dirty}
          previewCohort={actions.previewCohort}
          runOnCohort={actions.runOnCohort}
          registry={registry}
          publishedProfile={publishedProfile}
          onClose={() => setRunning(false)}
          text={text}
        />
      ) : null}
      <div className={r.workbench}>
        <div className="stack">
          <DraftChecks draft={draft} onResult={setCoreProblems} text={text} />
          <RegistryForm registry={registry} published={publishedProfile ?? {}} draft={draft} context="preview" errors={errors} onChange={editable ? setDraft : () => undefined} text={text} />
        </div>
        <div className={r.sticky}>
          <PreviewPanel
            registry={registry}
            publishedProfile={publishedProfile}
            baselines={baselines}
            catalog={catalog}
            draft={draft}
            sessions={sessions}
            loadRecords={actions.loadRecords}
            loadFrames={actions.loadFrames}
            text={text}
          />
        </div>
      </div>
    </div>
  );
}
