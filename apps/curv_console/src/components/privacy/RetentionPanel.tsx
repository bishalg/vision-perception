"use client";

import { useEffect, useRef, useState } from "react";
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
  | { kind: "working"; dryRun: boolean; status: string }
  | { kind: "done"; result: RetentionSweepResult }
  | { kind: "error"; failure: PrivacyFailure | null };

const POLL_MS = 1000;
const MAX_POLLS = 120;

/**
 * Preview (dry run) first, then Run sweep. Starting a sweep queues a job; this polls the job until it finishes and
 * shows the counts the backend computed. Nothing is counted or deleted here.
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
  const alive = useRef(true);
  useEffect(() => {
    alive.current = true;
    return () => {
      alive.current = false;
    };
  }, []);

  async function go(dryRun: boolean) {
    setConfirming(false);
    setPhase({ kind: "working", dryRun, status: "queued" });
    const started = await start(dryRun);
    if (!started.ok) return setPhase({ kind: "error", failure: started });
    for (let i = 0; i < MAX_POLLS && alive.current; i++) {
      await new Promise((r) => setTimeout(r, pollMs));
      if (!alive.current) return;
      const res = await poll(started.jobId);
      if (!res.ok) return setPhase({ kind: "error", failure: res });
      if (res.poll.state === "done") {
        const result = res.poll.result;
        if (result.dry_run) setPreviewCount(result.orgs.reduce((n, o) => n + o.frames_purged, 0));
        return setPhase({ kind: "done", result });
      }
      if (res.poll.state === "failed") return setPhase({ kind: "error", failure: null });
      setPhase({ kind: "working", dryRun, status: res.poll.status });
    }
  }

  const working = phase.kind === "working";
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
        <p role="status">{phase.dryRun ? t.running : t.runningReal}</p>
      ) : null}
      {phase.kind === "done" ? (
        <div className="stack" data-testid="sweep-result">
          <h3>{phase.result.dry_run ? t.resultDry : t.resultReal}</h3>
          {phase.result.orgs.length === 0 ? (
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
                  {phase.result.orgs.map((o) => (
                    <tr key={o.org_id}>
                      <td data-label={t.cols.months}>{o.retention_months}</td>
                      <td data-label={t.cols.cutoff}>{formatDateTime(o.cutoff)}</td>
                      <td data-label={t.cols.purged}>
                        {phase.result.dry_run ? t.purgedDry(o.frames_purged) : t.purgedReal(o.frames_purged)}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
        </div>
      ) : null}
      {phase.kind === "error" ? (
        <p role="alert" className="error">
          {t.failed} {phase.failure ? privacyFailureText(phase.failure) : t.failedJob}
        </p>
      ) : null}
    </section>
  );
}
