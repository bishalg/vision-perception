"use client";

import type { AuditEntry } from "@/lib/api";
import { formatDateTime } from "@/lib/format";
import { useText } from "@/lib/i18n/provider";

type AuditText = ReturnType<typeof useText>["app"]["audit"];

function json(value: unknown): string {
  return JSON.stringify(value, null, 2);
}

function hasValue(value: unknown): boolean {
  return value !== null && value !== undefined;
}

/** Before/after snapshots are redacted by the backend; shown as plain text (never as HTML). */
function Change({ entry, t }: { entry: AuditEntry; t: AuditText }) {
  if (!hasValue(entry.before) && !hasValue(entry.after)) return <>{t.noChange}</>;
  return (
    <details>
      <summary>{t.details}</summary>
      {hasValue(entry.before) ? (
        <>
          <strong>{t.before}</strong>
          <pre>{json(entry.before)}</pre>
        </>
      ) : null}
      {hasValue(entry.after) ? (
        <>
          <strong>{t.after}</strong>
          <pre>{json(entry.after)}</pre>
        </>
      ) : null}
    </details>
  );
}

const short = (id: string | null | undefined) => (id ? id.slice(0, 8) : "-");

export function AuditTable({ entries }: { entries: AuditEntry[] }) {
  const t = useText().app.audit;
  const c = t.cols;
  if (entries.length === 0) return <p className="muted">{t.empty}</p>;
  return (
    <div className="table-wrap">
      <table className="data responsive">
        <thead>
          <tr>
            <th scope="col">{c.time}</th>
            <th scope="col">{c.actor}</th>
            <th scope="col">{c.action}</th>
            <th scope="col">{c.target}</th>
            <th scope="col">{c.change}</th>
          </tr>
        </thead>
        <tbody>
          {entries.map((e) => (
            <tr key={e.id}>
              <td data-label={c.time}>{formatDateTime(e.ts)}</td>
              <td data-label={c.actor}>
                {t.actors[e.actor_type] ?? e.actor_type}
                {e.actor_id ? ` ${short(e.actor_id)}` : ""}
              </td>
              <td data-label={c.action}>{e.action}</td>
              <td data-label={c.target}>
                {e.target_type ? `${e.target_type} ${short(e.target_id)}` : "-"}
              </td>
              <td data-label={c.change}>
                <Change entry={e} t={t} />
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
