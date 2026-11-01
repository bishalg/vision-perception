import type { Metadata } from "next";
import Link from "next/link";
import { ExportForm } from "@/components/exports/ExportForm";
import { ExportsTable } from "@/components/exports/ExportsTable";
import { authedApi, currentRole } from "@/lib/auth/server";
import { parseCursor, toSearch } from "@/lib/listQuery";
import { loadOrNotFound } from "@/lib/loaders";
import { can } from "@/lib/permissions";
import { exportsText } from "@/messages/exports";
import { createExportAction } from "./actions";

const t = exportsText;

export const metadata: Metadata = { title: t.title };

export default async function ExportsPage({
  params,
  searchParams,
}: {
  params: Promise<{ org: string }>;
  searchParams: Promise<Record<string, string | string[] | undefined>>;
}) {
  const { org } = await params;
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
  const page = await loadOrNotFound(() => api.listExports(org, { cursor, limit: 50 }));
  // Every exporter role may pick from the active destinations (id, name, kind only; no config or secrets).
  const destinations = can(role, "export.create")
    ? (await loadOrNotFound(() => api.listExportDestinations(org))).items.filter((i) => i.status === "active")
    : [];
  const base = `/${encodeURIComponent(org)}/exports`;
  return (
    <>
      <h1>{t.title}</h1>
      <p className="muted">{t.intro}</p>
      <h2>{t.create.title}</h2>
      <ExportForm
        create={createExportAction.bind(null, org)}
        canCreate={can(role, "export.create")}
        canIdentified={can(role, "export.identified")}
        destinations={destinations}
        orgId={org}
      />
      <ExportsTable orgId={org} items={page.items} canDownload={can(role, "export.download")} />
      <p className="muted">{t.showing(page.items.length, page.total)}</p>
      <nav className="pagination" aria-label="Pagination">
        {cursor ? <Link href={base}>{t.first}</Link> : null}
        {page.next_cursor ? (
          <Link rel="next" href={`${base}${toSearch({ cursor: page.next_cursor })}`}>{t.next}</Link>
        ) : null}
      </nav>
    </>
  );
}
