/**
 * Version history list for a config document kind (WP-C.5): status filter, published version, new-draft link and a
 * paged table. Same layout as the catalog list (WP-C.4); server component.
 */
import Link from "next/link";
import { ConfigStatusBadge, authorText } from "@/components/catalog/CatalogTables";
import s from "@/components/catalog/catalog.module.css";
import { CONFIG_DOCUMENT_STATUSES, type ConfigDocument, type ConfigDocumentKind, type ConfigDocumentStatus } from "@/lib/api/catalog";
import type { Role } from "@/lib/api";
import { authedApi } from "@/lib/auth/server";
import { formatDateTime } from "@/lib/format";
import { parseCursor, toSearch } from "@/lib/listQuery";
import { loadOrNotFound } from "@/lib/loaders";
import { can } from "@/lib/permissions";
import { catalogText } from "@/messages/catalog";
import { configText } from "@/messages/config";

const t = configText;
const ct = catalogText;

export function DocVersionsTable({ base, items }: { base: string; items: ConfigDocument[] }) {
  if (!items.length) return <p className="muted">{ct.empty}</p>;
  return (
    <div className="table-wrap">
      <table className="data">
        <thead>
          <tr>
            <th>{ct.cols.version}</th>
            <th>{ct.cols.status}</th>
            <th>{ct.cols.created}</th>
            <th>{ct.cols.author}</th>
            <th>{ct.cols.published}</th>
            <th>{t.facts.notes}</th>
          </tr>
        </thead>
        <tbody>
          {items.map((d) => (
            <tr key={d.id}>
              <td><Link href={`${base}/${encodeURIComponent(d.version)}`}>{d.version}</Link></td>
              <td><ConfigStatusBadge status={d.status} /></td>
              <td>{formatDateTime(d.created_at)}</td>
              <td title={d.created_by ?? undefined}>{authorText(d.created_by)}</td>
              <td>{d.published_at ? formatDateTime(d.published_at) : "—"}</td>
              <td style={{ whiteSpace: "normal" }}>{d.notes ?? ""}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

export async function DocVersionsPage({
  org,
  kind,
  segment,
  title,
  intro,
  role,
  searchParams,
}: {
  org: string;
  kind: ConfigDocumentKind;
  segment: string;
  title: string;
  intro: string;
  role: Role | undefined;
  searchParams: Record<string, string | string[] | undefined>;
}) {
  if (!can(role, "config.read")) {
    return (
      <>
        <h1>{title}</h1>
        <p className="muted">{t.denied}</p>
      </>
    );
  }
  const cursor = parseCursor(searchParams);
  const rawStatus = Array.isArray(searchParams.status) ? searchParams.status[0] : searchParams.status;
  const status = (CONFIG_DOCUMENT_STATUSES as readonly string[]).includes(rawStatus ?? "")
    ? (rawStatus as ConfigDocumentStatus)
    : undefined;
  const api = await authedApi();
  const [page, live] = await Promise.all([
    loadOrNotFound(() => api.listConfigDocuments(kind, { status, cursor, limit: 50 })),
    loadOrNotFound(() => api.listConfigDocuments(kind, { status: "published", limit: 1 })),
  ]);
  const published = live.items[0];
  const base = `/${encodeURIComponent(org)}/config/${segment}`;
  const filterHref = (st?: string) => `${base}${toSearch({ status: st })}`;

  return (
    <>
      <h1>{title}</h1>
      <p className="muted">{intro}</p>
      <p>{published ? t.current(published.version) : t.noneLive}</p>
      {can(role, "config.manage") ? (
        <p>
          <Link className="button" href={`${base}/new${published ? toSearch({ from: published.version }) : ""}`}>
            {published ? t.newDraftFrom(published.version) : t.newDraft}
          </Link>
        </p>
      ) : (
        <p className="muted">{t.readOnly}</p>
      )}
      <nav className={`tabs ${s.filters}`} aria-label={ct.filterLabel}>
        <Link className={`tab${status ? "" : " active"}`} aria-current={status ? undefined : "page"} href={filterHref()}>
          {ct.all}
        </Link>
        {CONFIG_DOCUMENT_STATUSES.map((st) => (
          <Link key={st} className={`tab${status === st ? " active" : ""}`} aria-current={status === st ? "page" : undefined} href={filterHref(st)}>
            {ct.statuses[st]}
          </Link>
        ))}
      </nav>
      <DocVersionsTable base={base} items={page.items} />
      <p className="muted">{t.showing(page.items.length)}</p>
      <nav className="pagination" aria-label="Pagination">
        {cursor ? <Link href={filterHref(status)}>{ct.first}</Link> : null}
        {page.next_cursor ? (
          <Link rel="next" href={`${base}${toSearch({ status, cursor: page.next_cursor })}`}>{ct.next}</Link>
        ) : null}
      </nav>
    </>
  );
}
