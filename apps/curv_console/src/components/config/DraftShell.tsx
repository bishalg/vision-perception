"use client";

import { useRouter } from "next/navigation";
import { useEffect, useState, useTransition, type ReactNode } from "react";
import s from "@/components/catalog/catalog.module.css";
import { catalogFailureText, type CatalogResult } from "@/components/catalog/types";
import { browserCore } from "@/lib/asd/browser";
import { contractInfo, type ContractInfo, type Validation } from "@/lib/asd/ops";
import { serverFieldErrors } from "@/lib/catalog";
import type { AsdCore, Envelope } from "@/lib/asd/core";
import c from "./config.module.css";
import { useText } from "@/lib/i18n/provider";
import { pathsFor, type ConfigDocSegment } from "@/lib/paths";


export type CoreCheck =
  | { state: "checking" }
  | { state: "unavailable"; message: string }
  | { state: "done"; validation: Validation };

/**
 * Runs a Rust core check (asd-wasm) on `doc` whenever it changes (debounced). `doc === undefined` means "not
 * checkable yet" (for example a field with invalid JSON); the last result is kept.
 */
export function useCoreCheck<T>(doc: T | undefined, check: (core: AsdCore, doc: T) => Promise<Envelope<Validation>>): CoreCheck {
  const [result, setResult] = useState<CoreCheck>({ state: "checking" });
  const key = doc === undefined ? undefined : JSON.stringify(doc);
  useEffect(() => {
    if (key === undefined) return;
    let live = true;
    const timer = setTimeout(() => {
      void check(browserCore(), JSON.parse(key) as T).then((r) => {
        if (!live) return;
        if (r.ok) setResult({ state: "done", validation: r.result });
        else if (r.error.kind === "unavailable") setResult({ state: "unavailable", message: r.error.message });
        else setResult({ state: "done", validation: { valid: false, problems: [r.error.message], fieldProblems: [] } });
      });
    }, 150);
    return () => {
      live = false;
      clearTimeout(timer);
    };
    // `check` is a module-level function in every caller.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key]);
  return result;
}

/** The core's `contractInfo` (version, fixed low-confidence gate); `unavailable` when the module does not load. */
export function useContractInfo(): { info: ContractInfo | null; unavailable: boolean } {
  const [state, setState] = useState<{ info: ContractInfo | null; unavailable: boolean }>({ info: null, unavailable: false });
  useEffect(() => {
    let live = true;
    void contractInfo(browserCore()).then((r) => {
      if (!live) return;
      if (r.ok) setState({ info: r.result, unavailable: false });
      else if (r.error.kind === "unavailable") setState({ info: null, unavailable: true });
    });
    return () => {
      live = false;
    };
  }, []);
  return state;
}

export function CoreCheckPanel({ check, grouped }: { check: CoreCheck; grouped?: ReactNode }) {
  const tx = useText();
  const e = tx.config.editor;
  if (check.state === "checking") return <p className="muted" role="status">{e.checking}</p>;
  if (check.state === "unavailable") return <p className={s.alert} role="alert">{tx.config.sandbox.coreUnavailable}</p>;
  const v = check.validation;
  if (v.valid) return <p className={s.ok} role="status">{e.valid}</p>;
  return (
    <div className={s.alert} role="alert">
      {e.invalid(v.problems.length)}
      {grouped ?? (
        <ul className={c.problems}>
          {v.problems.map((p, i) => <li key={i}>{p}</li>)}
        </ul>
      )}
    </div>
  );
}

export interface DraftSubmit<T> {
  version: string;
  notes: string;
  body: T;
}

/**
 * Version, changelog note and "Save as draft" around a document editor. Saving needs a version, a note and a document
 * the Rust core accepts (`blocked` counts the problems); the server checks it again. Server field reasons come back
 * through `onServerFields`.
 */
export function DraftShell<T>({
  orgId,
  segment,
  fromVersion,
  body,
  blocked,
  save,
  onServerFields,
  children,
}: {
  orgId: string;
  segment: ConfigDocSegment;
  fromVersion: string | null;
  body: T | undefined;
  blocked: number;
  save: (input: DraftSubmit<T>) => Promise<CatalogResult<{ version: string }>>;
  onServerFields?: (fields: Record<string, string>) => void;
  children: ReactNode;
}) {
  const tx = useText();
  const e = tx.config.editor;
  const router = useRouter();
  const [version, setVersion] = useState("");
  const [notes, setNotes] = useState("");
  const [attempted, setAttempted] = useState(false);
  const [server, setServer] = useState<{ message: string; list: string[]; version?: string } | null>(null);
  const [pending, start] = useTransition();

  const versionError = attempted && !version.trim() ? e.versionRequired : server?.version;
  const notesError = attempted && !notes.trim() ? e.notesRequired : undefined;
  const local = (versionError ? 1 : 0) + (notesError ? 1 : 0) + (attempted ? blocked : 0) + (attempted && body === undefined ? 1 : 0);

  function submit() {
    setAttempted(true);
    if (!version.trim() || !notes.trim() || blocked || body === undefined) return;
    start(async () => {
      const r = await save({ version: version.trim(), notes: notes.trim(), body });
      if (r.ok) {
        router.push(`${pathsFor(orgId).config.docVersion(segment, r.version)}?created=1`);
        return;
      }
      const { fields, general } = serverFieldErrors(r.params ?? []);
      const versionReason = r.slug === "config-version-exists" ? catalogFailureText(r, tx.catalog.errors) : fields.version;
      delete fields.version;
      onServerFields?.(fields);
      setServer({
        message: catalogFailureText(r, tx.catalog.errors),
        list: [...general, ...Object.entries(fields).map(([k, v]) => `${k}: ${v}`), ...(versionReason ? [`version: ${versionReason}`] : [])],
        version: versionReason,
      });
    });
  }

  return (
    <div className="stack">
      <p className="muted">{fromVersion ? e.from(fromVersion) : e.blank}</p>
      <section className="panel stack" aria-label={e.details}>
        <div className={s.grid}>
          <label>
            {e.version}
            <input value={version} onChange={(ev) => setVersion(ev.currentTarget.value)} aria-invalid={Boolean(versionError) || undefined}
              aria-describedby={versionError ? "err-version" : undefined} className={versionError ? s.invalid : undefined} />
            <span className="muted">{e.versionHint}</span>
            {versionError ? <span id="err-version" className={s.fieldError}>{versionError}</span> : null}
          </label>
        </div>
        <label>
          {e.notes}
          <textarea rows={2} value={notes} maxLength={2000} onChange={(ev) => setNotes(ev.currentTarget.value)}
            aria-invalid={Boolean(notesError) || undefined} aria-describedby={notesError ? "err-notes" : undefined}
            className={notesError ? s.invalid : undefined} />
          {notesError ? <span id="err-notes" className={s.fieldError}>{notesError}</span> : null}
        </label>
      </section>

      {children}

      <div className={s.row}>
        <button type="button" disabled={pending} onClick={submit}>{pending ? e.saving : e.submit}</button>
        <button type="button" className="secondary" onClick={() => router.back()}>{e.cancel}</button>
      </div>
      {attempted && local ? <p role="alert" className={s.alert}>{e.fixFirst(local)}</p> : null}
      {server ? (
        <div role="alert" className={s.alert}>
          {server.message}
          {server.list.length ? (
            <>
              {" "}{e.serverRejected}
              <ul>{server.list.map((x, i) => <li key={i}>{x}</li>)}</ul>
            </>
          ) : null}
        </div>
      ) : null}
    </div>
  );
}
