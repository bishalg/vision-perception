"use client";

import Link from "next/link";
import type { Export, ExportStatus } from "@/lib/api/exports";
import { formatDateTime } from "@/lib/format";
import { useText } from "@/lib/i18n/provider";
import { pathsFor } from "@/lib/paths";

const STATUS_BADGE: Record<ExportStatus, string> = {
  queued: "badge-yellow",
  running: "badge-yellow",
  succeeded: "badge-green",
  failed: "badge-red",
  expired: "badge-unreliable",
};

export function ExportStatusBadge({ status }: { status: ExportStatus }) {
  const t = useText().exports.exports;
  return <span className={`badge ${STATUS_BADGE[status]}`}>{t.statuses[status]}</span>;
}

/** "Withdrawn 1, No research consent 3", or "None". Counts only. */
export function ExcludedSummary({ excluded }: { excluded: Export["excluded"] }) {
  const t = useText().exports.exports;
  const parts = Object.entries(excluded)
    .filter(([, n]) => n > 0)
    .map(([reason, n]) => `${t.excludedReasons[reason] ?? reason} ${n}`);
  return <>{parts.length ? parts.join(", ") : t.excludedNone}</>;
}

/**
 * Downloading is a POST to the console's own route (never a link): each download is audited by the backend and the
 * signed URL stays on the server.
 */
export function DownloadForm({ exportId }: { exportId: string }) {
  const t = useText().exports.exports;
  return (
    <form method="post" action={`/api/export/${encodeURIComponent(exportId)}`}>
      <button type="submit">{t.download}</button>
    </form>
  );
}

export function ExportsTable({ orgId, items, canDownload }: { orgId: string; items: Export[]; canDownload: boolean }) {
  const t = useText().exports.exports;
  const c = t.cols;
  if (items.length === 0) return <p className="muted">{t.empty}</p>;
  return (
    <div className="table-wrap">
      <table className="data responsive">
        <thead>
          <tr>
            <th scope="col">{c.created}</th>
            <th scope="col">{c.format}</th>
            <th scope="col">{c.mode}</th>
            <th scope="col">{c.purpose}</th>
            <th scope="col">{c.status}</th>
            <th scope="col">{c.sessions}</th>
            <th scope="col">{c.excluded}</th>
            <th scope="col">{c.expires}</th>
            <th scope="col">{c.actions}</th>
          </tr>
        </thead>
        <tbody>
          {items.map((e) => (
            <tr key={e.id}>
              <td data-label={c.created}>{formatDateTime(e.created_at)}</td>
              <td data-label={c.format}>{t.formats[e.format]}</td>
              <td data-label={c.mode}>{t.modes[e.mode]}</td>
              <td data-label={c.purpose}>{t.purposes[e.purpose]}</td>
              <td data-label={c.status}><ExportStatusBadge status={e.status} /></td>
              <td data-label={c.sessions}>{e.row_count ?? "-"}</td>
              <td data-label={c.excluded}><ExcludedSummary excluded={e.excluded} /></td>
              <td data-label={c.expires}>{e.expires_at ? formatDateTime(e.expires_at) : "-"}</td>
              <td data-label={c.actions}>
                <span className="confirm">
                  <Link href={pathsFor(orgId).data.export(e.id)}>{t.details}</Link>
                  {canDownload && e.status === "succeeded" ? <DownloadForm exportId={e.id} /> : null}
                </span>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
