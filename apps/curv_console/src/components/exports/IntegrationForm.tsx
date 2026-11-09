"use client";

import Link from "next/link";
import { useActionState, useState } from "react";
import {
  DELIVERY_EVENTS,
  DELIVERY_TRIGGERS,
  EXPORT_FORMATS,
  isDatasetFormat,
  isDedicatedFormat,
  EXPORT_MODES,
  INTEGRATION_ENVIRONMENTS,
  INTEGRATION_STATUSES,
  type Integration,
  type IntegrationKind,
} from "@/lib/api/exports";
import { useText } from "@/lib/i18n/provider";
import { failureText, type ActionResult } from "./types";
import { pathsFor } from "@/lib/paths";

/** Kinds that can be created today (the backend refuses the others until their channels exist). */
const CREATABLE: IntegrationKind[] = ["webhook", "sftp"];

type State = ActionResult<{ id?: string }> | null;

function Select<T extends string>({
  name,
  label,
  values,
  names,
  current,
  unset,
}: {
  name: string;
  label: string;
  values: readonly T[];
  names: Record<T, string>;
  current?: string | null;
  unset?: string;
}) {
  return (
    <label>
      {label}
      <select name={name} defaultValue={current ?? ""}>
        {unset ? <option value="">{unset}</option> : null}
        {values.map((v) => (
          <option key={v} value={v}>{names[v]}</option>
        ))}
      </select>
    </label>
  );
}

/**
 * Create (no `existing`) or edit a destination. Secrets are not part of this form: they have their own write-only
 * panels. On edit the whole config is re-sent (the API replaces it), so every config field is prefilled.
 */
export function IntegrationForm({
  save,
  orgId,
  existing,
}: {
  save: (formData: FormData) => Promise<ActionResult<{ id?: string }>>;
  orgId: string;
  existing?: Integration;
}) {
  const text = useText().exports;
  const t = text.integrations.form;
  const [state, action, pending] = useActionState<State, FormData>((_prev, fd) => save(fd), null);
  const [kind, setKind] = useState<IntegrationKind>(existing?.kind ?? "webhook");
  const cfg = existing?.config ?? {};
  const accepted = new Set<string>(cfg.events ?? DELIVERY_EVENTS);
  return (
    <form action={action} className="stack panel" aria-label={existing ? t.editTitle : t.createTitle}>
      <label>
        {t.name}
        <input name="name" required maxLength={120} defaultValue={existing?.name ?? ""} autoComplete="off" />
      </label>
      {existing ? (
        <input type="hidden" name="kind" value={existing.kind} />
      ) : (
        <label>
          {t.kind}
          <select name="kind" value={kind} onChange={(e) => setKind(e.target.value as IntegrationKind)}>
            {CREATABLE.map((k) => (
              <option key={k} value={k}>{text.integrations.kinds[k]}</option>
            ))}
          </select>
        </label>
      )}
      <Select
        name="environment"
        label={t.environment}
        values={INTEGRATION_ENVIRONMENTS}
        names={text.integrations.environments}
        current={existing?.environment ?? "test"}
      />
      <p className="muted">{t.environmentHint}</p>
      {existing ? (
        <Select name="status" label={t.status} values={INTEGRATION_STATUSES} names={text.integrations.statuses} current={existing.status} />
      ) : null}

      {kind === "webhook" ? (
        <>
          <label>
            {t.url}
            <input name="url" type="url" placeholder={t.urlPlaceholder} defaultValue={cfg.url ?? ""} autoComplete="off" />
          </label>
          <fieldset>
            <legend>{t.events}</legend>
            {DELIVERY_EVENTS.map((e) => (
              <label key={e} className="check">
                <input type="checkbox" name="events" value={e} defaultChecked={accepted.has(e)} />
                {text.deliveries.events[e]}
              </label>
            ))}
          </fieldset>
        </>
      ) : (
        <>
          <label>
            {t.host}
            <input name="host" maxLength={255} defaultValue={cfg.host ?? ""} autoComplete="off" />
          </label>
          <label>
            {t.port}
            <input name="port" type="number" min={1} max={65535} placeholder={t.portPlaceholder} defaultValue={cfg.port ?? ""} />
          </label>
          <label>
            {t.username}
            <input name="username" maxLength={64} defaultValue={cfg.username ?? ""} autoComplete="off" />
          </label>
          <label>
            {t.remoteDir}
            <input name="remote_dir" maxLength={255} placeholder={t.remoteDirPlaceholder} defaultValue={cfg.remote_dir ?? ""} autoComplete="off" />
          </label>
          <label>
            {t.hostKey}
            <input name="host_key_sha256" maxLength={100} defaultValue={cfg.host_key_sha256 ?? ""} autoComplete="off" />
          </label>
          <label>
            {t.filePrefix}
            <input name="file_prefix" maxLength={40} defaultValue={cfg.file_prefix ?? ""} autoComplete="off" />
          </label>
        </>
      )}
      <Select name="trigger" label={t.trigger} values={DELIVERY_TRIGGERS} names={text.integrations.triggers} current={cfg.trigger} unset={t.unset} />
      <Select name="format" label={t.format} values={EXPORT_FORMATS.filter((f) => !isDatasetFormat(f) && !isDedicatedFormat(f))} names={text.exports.formats} current={cfg.format} unset={t.unset} />
      <Select name="mode" label={t.mode} values={EXPORT_MODES} names={text.exports.modes} current={cfg.mode} unset={t.unset} />
      <div>
        <button type="submit" disabled={pending}>{existing ? t.save : t.create}</button>
      </div>
      {state?.ok ? (
        <p role="status">
          {existing ? t.saved : t.created}{" "}
          {state.id ? (
            <Link href={pathsFor(orgId).admin.integration(state.id)}>{t.openCreated}</Link>
          ) : null}
        </p>
      ) : null}
      {state && !state.ok ? <p role="alert" className="error">{t.failed} {failureText(state, text.exports.errors)}</p> : null}
    </form>
  );
}
