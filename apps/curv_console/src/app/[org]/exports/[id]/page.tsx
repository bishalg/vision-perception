import type { Metadata } from "next";
import Link from "next/link";
import { DeliveriesTable } from "@/components/exports/DeliveriesTable";
import { DownloadForm, ExportStatusBadge, excludedSummary } from "@/components/exports/ExportsTable";
import { authedApi, currentRole } from "@/lib/auth/server";
import { formatDateTime } from "@/lib/format";
import { parseCursor, toSearch } from "@/lib/listQuery";
import { loadOrNotFound } from "@/lib/loaders";
import { can } from "@/lib/permissions";
import { deliveriesText, exportsText } from "@/messages/exports";

const t = exportsText;
const d = t.detail;

// Fixed title: nothing from the export (reason, filters) reaches the document title.
export const metadata: Metadata = { title: t.title };

export default async function ExportDetailPage({
  params,
  searchParams,
}: {
  params: Promise<{ org: string; id: string }>;
  searchParams: Promise<Record<string, string | string[] | undefined>>;
}) {
  const { org, id } = await params;
  const role = await currentRole();
  if (!can(role, "export.read")) {
    return (
      <>
        <h1>{t.title}</h1>
        <p className="muted">{t.denied}</p>
      </>
    );
  }
  const cursor = parseCursor(await searchParams);
  const api = await authedApi();
  const e = await loadOrNotFound(() => api.getExport(id));
  const deliveries = await loadOrNotFound(() => api.listExportDeliveries(id, { cursor, limit: 50 }));
  const base = `/${encodeURIComponent(org)}/exports/${encodeURIComponent(id)}`;
  const filters = Object.entries(e.filters).filter(([, v]) => v != null && !(Array.isArray(v) && v.length === 0));
  return (
    <>
      <p><Link href={`/${encodeURIComponent(org)}/exports`}>{d.back}</Link></p>
      <h1>{t.title}</h1>
      <section className="panel stack" aria-label={d.summary}>
        <dl>
          <dt className="kpi-label">{d.id}</dt>
          <dd>{e.id}</dd>
          <dt className="kpi-label">{t.cols.status}</dt>
          <dd><ExportStatusBadge status={e.status} />{e.error ? <> <span className="error">{d.error}: {e.error}</span></> : null}</dd>
          <dt className="kpi-label">{t.cols.format}</dt>
          <dd>{t.formats[e.format]}</dd>
          <dt className="kpi-label">{t.cols.mode}</dt>
          <dd>{t.modes[e.mode]}</dd>
          <dt className="kpi-label">{t.cols.purpose}</dt>
          <dd>{t.purposes[e.purpose]}</dd>
          <dt className="kpi-label">{d.reason}</dt>
          <dd>{e.reason}</dd>
          <dt className="kpi-label">{t.cols.created}</dt>
          <dd>{formatDateTime(e.created_at)}</dd>
          {e.completed_at ? (
            <>
              <dt className="kpi-label">{d.completed}</dt>
              <dd>{formatDateTime(e.completed_at)}</dd>
            </>
          ) : null}
          <dt className="kpi-label">{t.cols.sessions}</dt>
          <dd>{e.row_count ?? "-"}</dd>
          <dt className="kpi-label">{t.cols.excluded}</dt>
          <dd>{excludedSummary(e.excluded)}</dd>
          <dt className="kpi-label">{t.cols.expires}</dt>
          <dd>{e.expires_at ? formatDateTime(e.expires_at) : "-"}</dd>
          <dt className="kpi-label">{d.filters}</dt>
          <dd>{filters.length === 0 ? d.noFilters : filters.map(([k, v]) => `${k}: ${Array.isArray(v) ? v.join(", ") : String(v)}`).join("; ")}</dd>
          {e.file_name ? (
            <>
              <dt className="kpi-label">{d.file}</dt>
              <dd>{e.file_name}{e.byte_size != null ? ` (${d.size}: ${e.byte_size} B)` : ""}</dd>
            </>
          ) : null}
          {e.sha256 ? (
            <>
              <dt className="kpi-label">{d.sha256}</dt>
              <dd><code>{e.sha256}</code></dd>
            </>
          ) : null}
        </dl>
        {can(role, "export.download") && e.status === "succeeded" ? (
          <>
            <DownloadForm exportId={e.id} />
            <p className="muted">{t.downloadHint}</p>
          </>
        ) : null}
      </section>
      <h2>{d.deliveries}</h2>
      {deliveries.items.length === 0 ? (
        <p className="muted">{d.noDeliveries}</p>
      ) : (
        <DeliveriesTable
          items={deliveries.items}
          destinationHref={
            can(role, "integration.read")
              ? (destinationId) => `/${encodeURIComponent(org)}/settings/integrations/${encodeURIComponent(destinationId)}`
              : undefined
          }
        />
      )}
      <p className="muted">{deliveriesText.showing(deliveries.items.length, deliveries.total)}</p>
      <nav className="pagination" aria-label="Pagination">
        {cursor ? <Link href={base}>{deliveriesText.first}</Link> : null}
        {deliveries.next_cursor ? (
          <Link rel="next" href={`${base}${toSearch({ cursor: deliveries.next_cursor })}`}>{deliveriesText.next}</Link>
        ) : null}
      </nav>
    </>
  );
}
