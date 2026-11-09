/**
 * Version history list for a config document kind (WP-C.5): status filter, published version, new-draft link and a
 * paged table. Same layout as the catalog list (WP-C.4); server component.
 */
import Link from "next/link";
import s from "@/components/catalog/catalog.module.css";
import { CONFIG_DOCUMENT_STATUSES, type ConfigDocumentKind, type ConfigDocumentStatus } from "@/lib/api/catalog";
import type { Role } from "@/lib/api";
import { authedApi } from "@/lib/auth/server";
import { getText } from "@/lib/i18n/server";
import { parseCursor, toSearch } from "@/lib/listQuery";
import { loadOrNotFound } from "@/lib/loaders";
import { can } from "@/lib/permissions";
import { DocVersionsTable } from "./DocVersionsTable";
import { pathsFor, type ConfigDocSegment } from "@/lib/paths";

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
  segment: ConfigDocSegment;
  title: string;
  intro: string;
  role: Role | undefined;
  searchParams: Record<string, string | string[] | undefined>;
}) {
  const tx = await getText();
  const t = tx.config;
  const ct = tx.catalog;
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
  const base = pathsFor(org).config.doc(segment);
  const filterHref = (st?: string) => `${base}${toSearch({ status: st })}`;

  return (
    <>
      <h1>{title}</h1>
      <p className="muted">{intro}</p>
      <p>{published ? t.current(published.version) : t.noneLive}</p>
      {can(role, "config.manage") ? (
        <p>
          <Link className="button" href={`${pathsFor(org).config.docNew(segment)}${published ? toSearch({ from: published.version }) : ""}`}>
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
      <DocVersionsTable org={org} segment={segment} items={page.items} />
      <p className="muted">{t.showing(page.items.length)}</p>
      <nav className="pagination" aria-label={t.pagination}>
        {cursor ? <Link href={filterHref(status)}>{ct.first}</Link> : null}
        {page.next_cursor ? (
          <Link rel="next" href={`${base}${toSearch({ status, cursor: page.next_cursor })}`}>{ct.next}</Link>
        ) : null}
      </nav>
    </>
  );
}
