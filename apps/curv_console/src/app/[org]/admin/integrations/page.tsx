import type { Metadata } from "next";
import Link from "next/link";
import { IntegrationForm } from "@/components/exports/IntegrationForm";
import { IntegrationsTable } from "@/components/exports/IntegrationsTable";
import { authedApi, currentRole } from "@/lib/auth/server";
import { parseCursor, toSearch } from "@/lib/listQuery";
import { loadOrNotFound } from "@/lib/loaders";
import { can } from "@/lib/permissions";
import { getText } from "@/lib/i18n/server";
import { createIntegrationAction } from "./actions";
import { pathsFor } from "@/lib/paths";

export async function generateMetadata(): Promise<Metadata> {
  return { title: (await getText()).exports.integrations.title };
}

export default async function IntegrationsPage({
  params,
  searchParams,
}: {
  params: Promise<{ org: string }>;
  searchParams: Promise<Record<string, string | string[] | undefined>>;
}) {
  const { org } = await params;
  const text = (await getText()).exports;
  const t = text.integrations;
  const role = await currentRole();
  if (!can(role, "integration.read")) {
    return (
      <>
        <h1>{t.title}</h1>
        <p className="muted">{t.denied}</p>
      </>
    );
  }
  const api = await authedApi();
  const cursor = parseCursor(await searchParams);
  const page = await loadOrNotFound(() => api.listIntegrations(org, { cursor, limit: 50 }));
  const base = pathsFor(org).admin.integrations;
  return (
    <>
      <h1>{t.title}</h1>
      <p className="muted">{t.intro}</p>
      <IntegrationsTable orgId={org} items={page.items} />
      <p className="muted">{t.showing(page.items.length, page.total)}</p>
      <nav className="pagination" aria-label={text.exports.pagination}>
        {cursor ? <Link href={base}>{t.first}</Link> : null}
        {page.next_cursor ? (
          <Link rel="next" href={`${base}${toSearch({ cursor: page.next_cursor })}`}>{t.next}</Link>
        ) : null}
      </nav>
      <h2>{t.form.createTitle}</h2>
      {can(role, "integration.manage") ? (
        <IntegrationForm save={createIntegrationAction.bind(null, org)} orgId={org} />
      ) : (
        <p className="muted">{t.readOnly}</p>
      )}
    </>
  );
}
