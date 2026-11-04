"use client";

import type { ErasureOutcome, ErasureRequest } from "@/lib/api/privacy";
import { formatDateTime } from "@/lib/format";
import { useText } from "@/lib/i18n/provider";
import type { Messages } from "@/lib/i18n/messages";

const BADGE = { pending: "badge-yellow", completed: "badge-green", held: "badge-unreliable" } as const;

function counts(o: ErasureOutcome, k: Messages["privacy"]["erasure"]["counts"]): string[] {
  const out: string[] = [];
  if (o.sessions_deleted) out.push(k.sessions_deleted(o.sessions_deleted));
  if (o.sessions_held) out.push(k.sessions_held(o.sessions_held));
  if (o.care_events_deleted) out.push(k.care_events_deleted(o.care_events_deleted));
  if (o.session_blobs_deleted) out.push(k.session_blobs_deleted(o.session_blobs_deleted));
  if (o.export_artifacts_deleted) out.push(k.export_artifacts_deleted(o.export_artifacts_deleted));
  if (o.clinical_drafts_deleted) out.push(k.clinical_drafts_deleted(o.clinical_drafts_deleted));
  if (o.patient_ref_deleted) out.push(k.patient_ref_deleted);
  return out;
}

/** Counts only: the backend never returns identifiers beyond the pseudonymous participant token. */
export function ErasureTable({ items }: { items: ErasureRequest[] }) {
  const t = useText().privacy.erasure;
  const c = t.cols;
  if (items.length === 0) return <p className="muted">{t.empty}</p>;
  return (
    <div className="table-wrap">
      <table className="data responsive">
        <thead>
          <tr>
            <th scope="col">{c.participant}</th>
            <th scope="col">{c.status}</th>
            <th scope="col">{c.requested}</th>
            <th scope="col">{c.by}</th>
            <th scope="col">{c.completed}</th>
            <th scope="col">{c.counts}</th>
          </tr>
        </thead>
        <tbody>
          {items.map((r) => {
            const lines = r.outcome ? counts(r.outcome, t.counts) : [];
            return (
              <tr key={r.id}>
                <td data-label={c.participant}>{r.participant_token}</td>
                <td data-label={c.status}>
                  <span className={`badge ${BADGE[r.status]}`}>{t.statuses[r.status]}</span>
                  {r.status === "held" ? (
                    <div className="muted">{t.holdReasons[r.hold_reason ?? ""] ?? t.statusHelp.held}</div>
                  ) : null}
                </td>
                <td data-label={c.requested}>{formatDateTime(r.requested_at)}</td>
                <td data-label={c.by}>{t.requestedBy[r.requested_by_type] ?? r.requested_by_type}</td>
                <td data-label={c.completed}>{r.completed_at ? formatDateTime(r.completed_at) : "-"}</td>
                <td data-label={c.counts}>
                  {r.outcome ? (lines.length ? lines.join(", ") : "-") : t.noCounts}
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}
