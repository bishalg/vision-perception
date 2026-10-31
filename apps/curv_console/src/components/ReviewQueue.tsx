"use client";

import Link from "next/link";
import { useRouter } from "next/navigation";
import { useCallback, useEffect, useRef, useState, useTransition } from "react";
import type { AttentionFlag, ReviewItem, ReviewStatus } from "@/lib/api";
import { formatAge, formatDateTime } from "@/lib/format";
import {
  CAPTURE_ISSUE_LABELS,
  TRIAGE_ACTIONS,
  isCaptureIssueLabel,
  triageForKey,
  type CaptureIssueLabel,
  type TriageAction,
} from "@/lib/review";
import { en } from "@/messages/en";
import { FlagBadge } from "./FlagBadge";

/** The session columns the queue shows next to each item (fetched by the page; null when unavailable). */
export interface ReviewSessionInfo {
  participant_token: string;
  attention_flag: AttentionFlag;
  quality_verdict: string;
  started_at: string;
}

export interface ReviewRow {
  item: ReviewItem;
  session: ReviewSessionInfo | null;
}

export type TriageResult = { ok: true; item: ReviewItem } | { ok: false; error: string };
export type TriageFn = (reviewId: string, action: TriageAction, label?: CaptureIssueLabel) => Promise<TriageResult>;

const t = en.review;

function isTyping(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  return target.isContentEditable || ["INPUT", "SELECT", "TEXTAREA"].includes(target.tagName);
}

export function ReviewQueue({
  orgId,
  // Initial rows only: the page re-keys this component when filters or the cursor change, and triage
  // updates the local copy, so a server refresh after an action does not need to reset it.
  rows: initialRows,
  canWrite,
  now,
  filterStatus,
  triage,
}: {
  orgId: string;
  rows: ReviewRow[];
  canWrite: boolean;
  /** Server time, so the SLA age renders the same on server and client. */
  now: number;
  /** The status the list is filtered by; items that leave it are removed after triage. */
  filterStatus?: ReviewStatus;
  triage: TriageFn;
}) {
  const router = useRouter();
  const [rows, setRows] = useState(initialRows);
  const [selected, setSelected] = useState(0);
  const [picking, setPicking] = useState(false);
  const [captureLabel, setCaptureLabel] = useState<CaptureIssueLabel>(CAPTURE_ISSUE_LABELS[0]);
  const [message, setMessage] = useState("");
  const [error, setError] = useState("");
  const [pending, startTransition] = useTransition();
  const pickerRef = useRef<HTMLSelectElement>(null);

  useEffect(() => {
    if (picking) pickerRef.current?.focus();
  }, [picking]);

  const current = rows[Math.min(selected, rows.length - 1)];
  const sessionHref = (r: ReviewRow) =>
    `/${encodeURIComponent(orgId)}/sessions/${encodeURIComponent(r.item.session_id)}`;

  const run = useCallback(
    (action: TriageAction, label?: CaptureIssueLabel) => {
      const target = rows[Math.min(selected, rows.length - 1)];
      if (!canWrite || !target || pending) return;
      setPicking(false);
      setError("");
      startTransition(async () => {
        const res = await triage(target.item.id, action, label);
        if (!res.ok) {
          setError(t.failed);
          return;
        }
        const who = target.session?.participant_token ?? target.item.session_id;
        setMessage(t.done(t.actions[action], who));
        const leaves = filterStatus !== undefined && res.item.status !== filterStatus;
        const remaining = leaves ? rows.length - 1 : rows.length;
        setRows((prev) =>
          leaves
            ? prev.filter((r) => r.item.id !== res.item.id)
            : prev.map((r) => (r.item.id === res.item.id ? { ...r, item: res.item } : r)),
        );
        // Move on to the next item: same index when the row left the list, the following one otherwise.
        setSelected((s) => Math.max(0, Math.min(leaves ? s : s + 1, remaining - 1)));
      });
    },
    [canWrite, filterStatus, pending, rows, selected, triage],
  );

  const choose = useCallback(
    (action: TriageAction) => {
      if (action === "capture_issue") {
        if (canWrite && current) setPicking(true);
      } else {
        run(action);
      }
    },
    [canWrite, current, run],
  );

  useEffect(() => {
    function onKey(e: KeyboardEvent) {
      if (e.metaKey || e.ctrlKey || e.altKey) return;
      if (picking && e.key === "Escape") {
        setPicking(false);
        return;
      }
      if (isTyping(e.target)) return;
      if (e.key === "j") setSelected((s) => Math.min(s + 1, Math.max(rows.length - 1, 0)));
      else if (e.key === "k") setSelected((s) => Math.max(s - 1, 0));
      else if (e.key === "o" && current) router.push(sessionHref(current));
      else {
        const action = triageForKey(e.key);
        if (!action) return;
        choose(action);
      }
      e.preventDefault();
    }
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
    // sessionHref is derived from orgId only.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [choose, current, picking, rows.length, router, orgId]);

  if (rows.length === 0) {
    return (
      <>
        <p className="muted">{t.empty}</p>
        <p role="status" className="sr-status">{message}</p>
      </>
    );
  }

  const c = t.cols;
  return (
    <>
      <p className="muted">{t.keys}</p>
      {canWrite ? (
        <div role="toolbar" aria-label={t.toolbar} className="toolbar">
          {TRIAGE_ACTIONS.map((a, i) => (
            <button key={a} type="button" disabled={pending || !current} onClick={() => choose(a)}>
              <kbd>{i + 1}</kbd> {t.actions[a]}
            </button>
          ))}
        </div>
      ) : (
        <p className="muted">{t.readOnly}</p>
      )}
      {picking ? (
        <form
          className="picker"
          onSubmit={(e) => {
            e.preventDefault();
            if (isCaptureIssueLabel(captureLabel)) run("capture_issue", captureLabel);
          }}
        >
          <label>
            {t.capturePick}
            <select
              ref={pickerRef}
              value={captureLabel}
              onChange={(e) => setCaptureLabel(e.target.value as CaptureIssueLabel)}
            >
              {CAPTURE_ISSUE_LABELS.map((l) => (
                <option key={l} value={l}>
                  {t.captureLabels[l]}
                </option>
              ))}
            </select>
          </label>
          <button type="submit" disabled={pending}>{t.apply}</button>
          <button type="button" className="link-button" onClick={() => setPicking(false)}>
            {t.cancel}
          </button>
        </form>
      ) : null}
      {error ? <p role="alert" className="error">{error}</p> : null}
      <p role="status" className="sr-status">{message}</p>
      <div className="table-wrap">
        <table className="data responsive review-table">
          <thead>
            <tr>
              <th scope="col">{c.session}</th>
              <th scope="col">{c.started}</th>
              <th scope="col">{c.flag}</th>
              <th scope="col">{c.quality}</th>
              <th scope="col">{c.reason}</th>
              <th scope="col">{c.status}</th>
              <th scope="col" className="num">{c.age}</th>
            </tr>
          </thead>
          <tbody>
            {rows.map((r, i) => {
              const isSel = r === current;
              return (
                <tr
                  key={r.item.id}
                  aria-current={isSel ? "true" : undefined}
                  className={isSel ? "selected" : undefined}
                  onClick={() => setSelected(i)}
                >
                  <td data-label={c.session}>
                    <Link href={sessionHref(r)}>{r.session?.participant_token ?? r.item.session_id}</Link>
                  </td>
                  <td data-label={c.started}>{r.session ? formatDateTime(r.session.started_at) : "-"}</td>
                  <td data-label={c.flag}>{r.session ? <FlagBadge flag={r.session.attention_flag} /> : "-"}</td>
                  <td data-label={c.quality}>{r.session?.quality_verdict ?? "-"}</td>
                  <td data-label={c.reason}>{t.reasons[r.item.reason]}</td>
                  <td data-label={c.status}>{t.statuses[r.item.status]}</td>
                  <td data-label={c.age} className="num">{formatAge(r.item.created_at, now)}</td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>
    </>
  );
}
