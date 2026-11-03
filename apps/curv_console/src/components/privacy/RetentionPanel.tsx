"use client";

import { useState } from "react";
import { useJobPoll } from "@/hooks/useJobPoll";
import type { RetentionSweepResult } from "@/lib/api/privacy";
import { formatDateTime } from "@/lib/format";
import { privacyText } from "@/messages/privacy";
import { privacyFailureText, type PrivacyFailure, type PrivacyResult } from "./types";

const t = privacyText.retention;

export type SweepPollResult =
  | { state: "pending"; status: string }
  | { state: "done"; result: RetentionSweepResult }
  | { state: "failed" };

type Phase =
  | { kind: "idle" }
  | { kind: "starting"; dryRun: boolean }
  | { kind: "job"; jobId: string; dryRun: boolean }
  | { kind: "error"; failure: PrivacyFailure | null };

const POLL_MS = 1000;
const MAX_POLLS = 120;

/**
 * Preview (dry run) first, then Run sweep. Starting a sweep queues a job; `useJobPoll` polls the job until it finishes
 * and the panel shows the counts the backend computed. Nothing is counted or deleted here.
 */
export function RetentionPanel({
  start,
  poll,
  pollMs = POLL_MS,
}: {
  start: (dryRun: boolean) => Promise<PrivacyResult<{ jobId: string }>>;
  poll: (jobId: string) => Promise<PrivacyResult<{ poll: SweepPollResult }>>;
  pollMs?: number;
}) {
  const [phase, setPhase] = useState<Phase>({ kind: "idle" });
  const [confirming, setConfirming] = useState(false);
  const [previewCount, setPreviewCount] = useState<number | null>(null);
  const jobId = phase.kind === "job" ? phase.jobId : "";
  const job = useJobPoll<SweepPollResult>({
    enabled: phase.kind === "job",
    jobKey: jobId,
    intervalMs: pollMs,
    maxPolls: MAX_POLLS,
    fetch: async () => {
      const res = await poll(jobId);
      if (!res.ok) throw res;
      return res.poll;
    },
    isFinal: (p) => p.state !== "pending",
    onValue: (p) => {
      if (p.state === "done" && p.result.dry_run) setPreviewCount(p.result.orgs.reduce((n, o) => n + o.frames_purged, 0));
    },
  });

  async function go(dryRunNext: boolean) {
    setConfirming(false);
    setPhase({ kind: "starting", dryRun: dryRunNext });
    const started = await start(dryRunNext);
    setPhase(started.ok ? { kind: "job", jobId: started.jobId, dryRun: dryRunNext } : { kind: "error", failure: started });
  }

  // The job's latest answer wins over the phase that started it.
  const answer = job.value;
  const result = answer?.state === "done" ? answer.result : null;
  const failed: { failure: PrivacyFailure | null } | null =
    job.state === "error"
      ? { failure: job.error as PrivacyFailure }
      : answer?.state === "failed" || job.state === "timeout"
        ? { failure: null }
        : phase.kind === "error"
          ? { failure: phase.failure }
          : null;
  const dryRun = phase.kind === "starting" || phase.kind === "job" ? phase.dryRun : true;
  const working = (phase.kind === "starting" || phase.kind === "job") && !result && !failed;
  return (
    <section className="panel stack" aria-label={t.title}>
      <p className="muted">{t.intro}</p>
      <div className="filter-actions">
        <button type="button" onClick={() => go(true)} disabled={working}>{t.preview}</button>
        {!confirming ? (
          <button type="button" className="danger" onClick={() => setConfirming(true)} disabled={working}>{t.run}</button>
        ) : null}
      </div>
      <p className="muted">{t.runHint}</p>
      {confirming ? (
        <div className="stack">
          <p role="alertdialog" aria-label={t.run}>{t.confirm(previewCount)}</p>
          <div className="filter-actions">
            <button type="button" className="danger" onClick={() => go(false)} disabled={working}>{t.confirmButton}</button>
            <button type="button" onClick={() => setConfirming(false)}>{t.cancel}</button>
          </div>
        </div>
      ) : null}
      {working ? (
        <p role="status">{dryRun ? t.running : t.runningReal}</p>
      ) : null}
      {result ? (
        <div className="stack" data-testid="sweep-result">
          <h3>{result.dry_run ? t.resultDry : t.resultReal}</h3>
          {result.orgs.length === 0 ? (
            <p className="muted">{t.noOrgs}</p>
          ) : (
            <div className="table-wrap">
              <table className="data responsive">
                <thead>
                  <tr>
                    <th scope="col">{t.cols.months}</th>
                    <th scope="col">{t.cols.cutoff}</th>
                    <th scope="col">{t.cols.purged}</th>
                  </tr>
                </thead>
                <tbody>
                  {result.orgs.map((o) => (
                    <tr key={o.org_id}>
                      <td data-label={t.cols.months}>{o.retention_months}</td>
                      <td data-label={t.cols.cutoff}>{formatDateTime(o.cutoff)}</td>
                      <td data-label={t.cols.purged}>
                        {result.dry_run ? t.purgedDry(o.frames_purged) : t.purgedReal(o.frames_purged)}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
        </div>
      ) : null}
      {failed ? (
        <p role="alert" className="error">
          {t.failed} {failed.failure ? privacyFailureText(failed.failure) : t.failedJob}
        </p>
      ) : null}
    </section>
  );
}
