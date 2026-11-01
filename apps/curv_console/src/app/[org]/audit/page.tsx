import type { Metadata } from "next";
import Link from "next/link";
import { AuditFilters } from "@/components/AuditFilters";
import { AuditTable } from "@/components/AuditTable";
import { authedApi, currentRole } from "@/lib/auth/server";
import { auditFilterParams, parseAuditQuery, toSearch } from "@/lib/listQuery";
import { loadOrNotFound } from "@/lib/loaders";
import { can } from "@/lib/permissions";
import { adminText } from "@/messages/admin";

const t = adminText.audit;

export const metadata: Metadata = { title: t.title };

export default async function AuditPage({
  params,
  searchParams,
}: {
  params: Promise<{ org: string }>;
  searchParams: Promise<Record<string, string | string[] | undefined>>;
}) {
  const { org } = await params;
  if (!can(await currentRole(), "audit.read")) {
    return (
      <>
        <h1>{t.title}</h1>
        <p className="muted">{adminText.common.denied}</p>
      </>
    );
  }
  const query = parseAuditQuery(await searchParams);
  const { cursor } = query;
  const filters = auditFilterParams(query);
  const filtered = Object.values(filters).some(Boolean);
  const api = await authedApi();
  const page = await loadOrNotFound(() => api.listAudit(org, { ...query, limit: 50 }));
  const base = `/${encodeURIComponent(org)}/audit`;
  return (
    <>
      <h1>{t.title}</h1>
      <p className="muted">{t.intro}</p>
      <AuditFilters orgId={org} query={query} />
      {filtered && page.items.length === 0 ? (
        <p className="muted">{t.filters.emptyFiltered}</p>
      ) : (
        <AuditTable entries={page.items} />
      )}
      <p className="muted">{t.showing(page.items.length)}</p>
      <nav className="pagination" aria-label="Pagination">
        {cursor ? <Link href={`${base}${toSearch(filters)}`}>{adminText.common.first}</Link> : null}
        {page.next_cursor ? (
          <Link rel="next" href={`${base}${toSearch({ ...filters, cursor: page.next_cursor })}`}>{adminText.common.next}</Link>
        ) : null}
      </nav>
    </>
  );
}
