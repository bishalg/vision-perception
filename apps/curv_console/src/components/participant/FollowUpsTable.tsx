"use client";

import Link from "next/link";
import { useState, useTransition } from "react";
import type { FollowUp } from "@/lib/api/emr";
import { formatDateTime } from "@/lib/format";
import { useText } from "@/lib/i18n/provider";
import { failureText, type ActionResult } from "./types";

/** Earliest due first, whatever order the rows arrived in (the API already sorts; this keeps the page honest). */
export function sortFollowUps(rows: FollowUp[]): FollowUp[] {
  return [...rows].sort(
    (a, b) => Date.parse(a.follow_up_due_at) - Date.parse(b.follow_up_due_at) || a.encounter_id.localeCompare(b.encounter_id),
  );
}

const DAY = 86_400_000;

function DoneButton({ token, done }: { token: string; done: () => Promise<ActionResult> }) {
  const text = useText();
  const t = text.participant.followUps;
  const [busy, start] = useTransition();
  const [result, setResult] = useState<ActionResult | null>(null);
  return (
    <>
      <button
        type="button"
        className="secondary"
        disabled={busy}
        aria-label={t.markDoneLabel(token)}
        onClick={() => start(async () => setResult(await done().catch(() => ({ ok: false, slug: null, status: 0 }) as const)))}
      >
        {t.markDone}
      </button>
      {result && !result.ok ? <span role="alert" className="error"> {t.failed} {failureText(result, text.locale)}</span> : null}
    </>
  );
}

/** The follow-ups list (REC-4): due date first, with an overdue badge, the participant link and "Mark done". */
export function FollowUpsTable({
  rows,
  now,
  orgPath,
  canWrite,
  done,
  filtered,
}: {
  rows: FollowUp[];
  /** Request time (ms) for the overdue / today badges. */
  now: number;
  orgPath: string;
  canWrite: boolean;
  done: (encounterId: string) => Promise<ActionResult>;
  filtered: boolean;
}) {
  const text = useText();
  const t = text.participant.followUps;
  const c = t.cols;
  if (rows.length === 0) return <p className="muted">{filtered ? t.emptyFiltered : t.empty}</p>;
  return (
    <div className="table-wrap">
      <table className="data responsive" aria-label={t.title}>
        <thead>
          <tr>
            <th scope="col">{c.due}</th>
            <th scope="col">{c.participant}</th>
            <th scope="col">{c.visit}</th>
            <th scope="col">{c.reason}</th>
            <th scope="col">{c.staff}</th>
            {canWrite ? <th scope="col">{c.actions}</th> : null}
          </tr>
        </thead>
        <tbody>
          {sortFollowUps(rows).map((f) => {
            const due = Date.parse(f.follow_up_due_at);
            const overdue = due < now;
            const today = !overdue && due - now < DAY;
            return (
              <tr key={f.encounter_id} data-testid="follow-up" data-overdue={overdue ? "true" : "false"}>
                <td data-label={c.due}>
                  {formatDateTime(f.follow_up_due_at)}{" "}
                  {overdue ? <span className="badge badge-red">{t.overdueBadge}</span> : today ? <span className="badge badge-yellow">{t.todayBadge}</span> : null}
                </td>
                <td data-label={c.participant}>
                  <Link href={`${orgPath}/participants/${encodeURIComponent(f.participant_id)}?tab=visits`}>{f.participant_token}</Link>
                </td>
                <td data-label={c.visit}>
                  {text.participant.visits.visitLine(text.domain.enumLabel(text.locale, "encounter_kind", f.kind), formatDateTime(f.occurred_at))}
                </td>
                <td data-label={c.reason}>{f.reason ?? "-"}</td>
                <td data-label={c.staff}>{f.clinician_display_name ?? "-"}</td>
                {canWrite ? (
                  <td data-label={c.actions}>
                    <DoneButton token={f.participant_token} done={() => done(f.encounter_id)} />
                  </td>
                ) : null}
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}
