"use client";

import { useEffect } from "react";
import s from "@/components/catalog/catalog.module.css";
import { useCoreCheck } from "@/components/config/DraftShell";
import { validateScoringProfile } from "@/lib/asd/ops";
import type { ProfileDoc } from "@/lib/research/registry";
import { researchText, type ResearchMessages } from "@/messages/research";

export interface CoreProblems {
  count: number;
  fields: Record<string, string>;
}

/**
 * Checks the draft with the Rust core in the browser (asd-wasm `validateScoringProfile`, the backend's own range
 * checks) after every change and reports the problems per parameter path to the workbench; shows a one-line status.
 */
export function DraftChecks({
  draft,
  onResult,
  text = researchText,
}: {
  draft: ProfileDoc;
  onResult: (p: CoreProblems) => void;
  text?: ResearchMessages;
}) {
  const check = useCoreCheck(draft, validateScoringProfile);
  const w = text.workbench;
  useEffect(() => {
    if (check.state !== "done") {
      onResult({ count: 0, fields: {} });
      return;
    }
    const fields: Record<string, string> = {};
    for (const fp of check.validation.fieldProblems) fields[fp.path] = fields[fp.path] ? `${fields[fp.path]}; ${fp.message}` : fp.message;
    onResult({ count: check.validation.problems.length, fields });
    // `onResult` is a state setter of the workbench.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [check]);

  if (check.state === "checking") return <p className="muted" role="status">{w.checking}</p>;
  if (check.state === "unavailable") return <p className="muted" role="status">{w.coreUnavailable}</p>;
  const v = check.validation;
  if (v.valid) return <p className={s.ok} role="status">{w.valid}</p>;
  return (
    <div className={s.alert} role="alert">
      {w.invalid(v.problems.length)}
      <ul>{v.problems.map((m, i) => <li key={i}>{m}</li>)}</ul>
    </div>
  );
}
