import type { Metadata } from "next";
import Link from "next/link";
import { AuditTable } from "@/components/AuditTable";
import { authedApi, currentRole } from "@/lib/auth/server";
import { parseCursor, toSearch } from "@/lib/listQuery";
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
  const cursor = parseCursor(await searchParams);
  const api = await authedApi();
  const page = await loadOrNotFound(() => api.listAudit(org, { cursor, limit: 50 }));
  const base = `/${encodeURIComponent(org)}/audit`;
  return (
    <>
      <h1>{t.title}</h1>
      <p className="muted">{t.intro}</p>
      <AuditTable entries={page.items} />
      <p className="muted">{t.showing(page.items.length)}</p>
      <nav className="pagination" aria-label="Pagination">
        {cursor ? <Link href={base}>{adminText.common.first}</Link> : null}
        {page.next_cursor ? (
          <Link rel="next" href={`${base}${toSearch({ cursor: page.next_cursor })}`}>{adminText.common.next}</Link>
        ) : null}
      </nav>
    </>
  );
}
