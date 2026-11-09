import type { Metadata } from "next";
import Link from "next/link";
import { AuditFilters } from "@/components/AuditFilters";
import { AuditTable } from "@/components/AuditTable";
import { exportAuditCsvAction } from "./actions";
import { authedApi, currentRole } from "@/lib/auth/server";
import { auditFilterParams, parseAuditQuery, toSearch } from "@/lib/listQuery";
import { loadOrNotFound } from "@/lib/loaders";
import { can } from "@/lib/permissions";
import { getText } from "@/lib/i18n/server";
import { pathsFor } from "@/lib/paths";

export async function generateMetadata(): Promise<Metadata> {
  return { title: (await getText()).app.audit.title };
}

export default async function AuditPage({
  params,
  searchParams,
}: {
  params: Promise<{ org: string }>;
  searchParams: Promise<Record<string, string | string[] | undefined>>;
}) {
  const { org } = await params;
  const text = await getText();
  const t = text.app.audit;
  if (!can(await currentRole(), "audit.read")) {
    return (
      <>
        <h1>{t.title}</h1>
        <p className="muted">{text.app.common.denied}</p>
      </>
    );
  }
  const sp = await searchParams;
  const query = parseAuditQuery(sp);
  const exportError = typeof sp.export_error === "string" ? sp.export_error : null;
  // HIP-3: AuditExport on top of the route's export action (the superadmin has the first but never the second).
  const role = await currentRole();
  const canExport = can(role, "audit.export") && can(role, "export.create");
  const { cursor } = query;
  const filters = auditFilterParams(query);
  const filtered = Object.values(filters).some(Boolean);
  const api = await authedApi();
  const page = await loadOrNotFound(() => api.listAudit(org, { ...query, limit: 50 }));
  const base = pathsFor(org).data.audit;
  return (
    <>
      <h1>{t.title}</h1>
      <p className="muted">{t.intro}</p>
      <AuditFilters orgId={org} query={query} />
      {canExport ? (
        <form action={exportAuditCsvAction.bind(null, org)} className="filters" aria-label={t.export.label}>
          <input type="hidden" name="actor_id" value={filters.actor_id ?? ""} />
          <input type="hidden" name="action_prefix" value={filters.action ?? ""} />
          <input type="hidden" name="from" value={filters.from ?? ""} />
          <input type="hidden" name="to" value={filters.to ?? ""} />
          <label>
            {t.export.reason}
            <input type="text" name="reason" required minLength={10} maxLength={500} defaultValue={t.export.reasonDefault} autoComplete="off" />
          </label>
          <p className="muted">{t.export.reasonHelp}</p>
          <div className="filter-actions">
            <button type="submit">{t.export.button}</button>
          </div>
          {exportError ? <p role="alert">{t.export.errors[exportError] ?? t.export.errorFallback}</p> : null}
        </form>
      ) : null}
      {filtered && page.items.length === 0 ? (
        <p className="muted">{t.filters.emptyFiltered}</p>
      ) : (
        <AuditTable entries={page.items} />
      )}
      <p className="muted">{t.showing(page.items.length)}</p>
      <nav className="pagination" aria-label={text.app.common.pagination}>
        {cursor ? <Link href={`${base}${toSearch(filters)}`}>{text.app.common.first}</Link> : null}
        {page.next_cursor ? (
          <Link rel="next" href={`${base}${toSearch({ ...filters, cursor: page.next_cursor })}`}>{text.app.common.next}</Link>
        ) : null}
      </nav>
    </>
  );
}
