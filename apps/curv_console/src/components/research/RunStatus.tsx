"use client";

import Link from "next/link";
import { useRouter } from "next/navigation";
import { useState, useTransition } from "react";
import { useJobPoll } from "@/hooks/useJobPoll";
import { isRunFinished } from "@/lib/api/research";
import { useText } from "@/lib/i18n/provider";
import { failureText, type ResearchResult } from "@/lib/research/result";
import { exportPrefillHref, type RunSnapshot } from "@/lib/research/runs";
import type { ResearchMessages } from "@/messages/research";
import r from "./research.module.css";

const POLL_MS = 1000;

/**
 * The live status of one run (RUI-6): polls `getAnalysisRun` (through a server action) while the run is queued or
 * running, stops on any final state and on unmount, and refreshes the page once when the run ends so the results load.
 * Cancel shows only while the run can still be cancelled; "Run again" and the export pre-fill only after it ended.
 */
export function RunStatus({
  orgId,
  runId,
  initial,
  poll,
  cancel,
  rerun,
  canRun,
  canExport,
  pollMs = POLL_MS,
  text: textProp,
}: {
  orgId: string;
  runId: string;
  initial: RunSnapshot;
  poll: (runId: string) => Promise<ResearchResult<{ run: RunSnapshot }>>;
  cancel: (runId: string) => Promise<ResearchResult<{ run: RunSnapshot }>>;
  rerun: (runId: string) => Promise<ResearchResult<{ runId: string }>>;
  canRun: boolean;
  canExport: boolean;
  pollMs?: number;
  text?: ResearchMessages;
}) {
  const bundle = useText();
  const text = textProp ?? bundle.research;
  const t = text.runs;
  const router = useRouter();
  const [override, setOverride] = useState<RunSnapshot | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [pending, start] = useTransition();
  const job = useJobPoll<RunSnapshot>({
    enabled: !isRunFinished(initial.status) && override === null,
    jobKey: runId,
    intervalMs: pollMs,
    maxPolls: 600,
    fetch: async () => {
      const res = await poll(runId);
      if (!res.ok) throw res;
      return res.run;
    },
    isFinal: (s) => isRunFinished(s.status),
    onValue: (s) => {
      if (isRunFinished(s.status)) router.refresh();
    },
  });
  // A page re-render can bring a final state before the poll sees it; a final state is terminal, so it wins.
  const current = override ?? (isRunFinished(initial.status) ? initial : (job.value ?? initial));
  const finished = isRunFinished(current.status);
  const stopped = job.state === "error" || job.state === "timeout";

  function doCancel() {
    setError(null);
    start(async () => {
      const res = await cancel(runId);
      if (res.ok) {
        // A queued run ends `cancelled` at once; a running one answers 202 still `running` (cancel requested) and
        // stops before its next page, so the poll keeps going until it sees the final state.
        if (isRunFinished(res.run.status)) {
          setOverride(res.run);
          router.refresh();
        }
      } else setError(failureText(res, text));
    });
  }

  function doRerun() {
    setError(null);
    start(async () => {
      const res = await rerun(runId);
      if (res.ok) router.push(`/${encodeURIComponent(orgId)}/research/runs/${encodeURIComponent(res.runId)}`);
      else setError(failureText(res, text));
    });
  }

  const code = current.error;
  const failureCopy = code === "budget_exceeded" || code === "result-too-large" ? t.failure[code] : t.failure.fallback;
  return (
    <section className="panel stack" aria-label={t.cols.status} data-run-status={current.status}>
      <div className={r.statusRow}>
        <strong role="status" data-testid="run-status">{text.runStatus[current.status]}</strong>
        {current.progress && !finished ? (
          <>
            <progress className={r.progress} max={Math.max(current.progress.total, 1)} value={current.progress.done} aria-label={t.progress(current.progress.done, current.progress.total, current.progress.phase)} />
            <span className="muted">{t.progress(current.progress.done, current.progress.total, current.progress.phase)}</span>
          </>
        ) : null}
        {!finished && !stopped ? <span className="muted">{t.updating}</span> : null}
      </div>
      {stopped ? <p role="alert" className="muted">{t.pollStopped}</p> : null}
      {current.status === "failed" ? (
        <div role="alert" className="stack">
          <p className="error">{t.failure.title} {failureCopy}</p>
          {code ? <p className="muted"><code>{t.failure.code(code)}</code></p> : null}
        </div>
      ) : null}
      {current.status === "cancelled" ? <p className="muted">{t.cancelled}</p> : null}
      <div className={r.statusRow}>
        {canRun && !finished ? (
          <button type="button" className="danger" disabled={pending} onClick={doCancel}>{pending ? t.cancelling : t.cancel}</button>
        ) : null}
        {canRun && finished ? (
          <button type="button" className="secondary" disabled={pending} onClick={doRerun}>{pending ? t.rerunning : t.rerun}</button>
        ) : null}
        {canExport && current.status === "succeeded" ? <Link href={exportPrefillHref(orgId, runId)}>{t.exportResults}</Link> : null}
      </div>
      {error ? <p role="alert" className="error">{error}</p> : null}
    </section>
  );
}
