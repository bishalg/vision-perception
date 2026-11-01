import type { Metadata } from "next";
import Link from "next/link";
import { VersionsTable } from "@/components/catalog/CatalogTables";
import s from "@/components/catalog/catalog.module.css";
import { CONFIG_DOCUMENT_STATUSES, type ConfigDocumentStatus } from "@/lib/api/catalog";
import { authedApi, currentRole } from "@/lib/auth/server";
import { parseCursor, toSearch } from "@/lib/listQuery";
import { loadOrNotFound } from "@/lib/loaders";
import { can } from "@/lib/permissions";
import { catalogText } from "@/messages/catalog";

const t = catalogText;

export const metadata: Metadata = { title: t.title };

export default async function CatalogPage({
  params,
  searchParams,
}: {
  params: Promise<{ org: string }>;
  searchParams: Promise<Record<string, string | string[] | undefined>>;
}) {
  const { org } = await params;
  const role = await currentRole();
  if (!can(role, "config.read")) {
    return (
      <>
        <h1>{t.title}</h1>
        <p className="muted">{t.denied}</p>
      </>
    );
  }
  const raw = await searchParams;
  const cursor = parseCursor(raw);
  const rawStatus = Array.isArray(raw.status) ? raw.status[0] : raw.status;
  const status = (CONFIG_DOCUMENT_STATUSES as readonly string[]).includes(rawStatus ?? "")
    ? (rawStatus as ConfigDocumentStatus)
    : undefined;
  const api = await authedApi();
  const [page, live] = await Promise.all([
    loadOrNotFound(() => api.listConfigDocuments("stimulus_catalog", { status, cursor, limit: 50 })),
    loadOrNotFound(() => api.listConfigDocuments("stimulus_catalog", { status: "published", limit: 1 })),
  ]);
  const published = live.items[0];
  const base = `/${encodeURIComponent(org)}/config/catalog`;
  const canManage = can(role, "config.manage");
  const filterHref = (st?: string) => `${base}${toSearch({ status: st })}`;

  return (
    <>
      <h1>{t.title}</h1>
      <p className="muted">{t.intro}</p>
      <p>{published ? t.current(published.version) : t.noneLive}</p>
      {canManage ? (
        <p>
          <Link className="button" href={`${base}/new${published ? toSearch({ from: published.version }) : ""}`}>
            {published ? t.newDraftFrom(published.version) : t.newDraft}
          </Link>
        </p>
      ) : (
        <p className="muted">{t.readOnly}</p>
      )}
      <nav className={`tabs ${s.filters}`} aria-label={t.filterLabel}>
        <Link className={`tab${status ? "" : " active"}`} aria-current={status ? undefined : "page"} href={filterHref()}>
          {t.all}
        </Link>
        {CONFIG_DOCUMENT_STATUSES.map((st) => (
          <Link key={st} className={`tab${status === st ? " active" : ""}`} aria-current={status === st ? "page" : undefined} href={filterHref(st)}>
            {t.statuses[st]}
          </Link>
        ))}
      </nav>
      <VersionsTable orgId={org} items={page.items} />
      <p className="muted">{t.showing(page.items.length)}</p>
      <nav className="pagination" aria-label="Pagination">
        {cursor ? <Link href={filterHref(status)}>{t.first}</Link> : null}
        {page.next_cursor ? (
          <Link rel="next" href={`${base}${toSearch({ status, cursor: page.next_cursor })}`}>{t.next}</Link>
        ) : null}
      </nav>
    </>
  );
}
