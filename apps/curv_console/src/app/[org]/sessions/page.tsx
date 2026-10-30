import type { Metadata } from "next";
import { Pagination } from "@/components/Pagination";
import { SessionFilters } from "@/components/SessionFilters";
import { SessionsTable } from "@/components/SessionsTable";
import { authedApi } from "@/lib/auth/server";
import { loadOrNotFound } from "@/lib/loaders";
import { parseSessionQuery } from "@/lib/sessionQuery";
import { en } from "@/messages/en";

export const metadata: Metadata = { title: en.sessions.title };

export default async function SessionsPage({
  params,
  searchParams,
}: {
  params: Promise<{ org: string }>;
  searchParams: Promise<Record<string, string | string[] | undefined>>;
}) {
  const { org } = await params;
  const query = parseSessionQuery(await searchParams);
  const api = await authedApi();
  const page = await loadOrNotFound(() => api.listSessions(org, query));
  return (
    <>
      <h1>{en.sessions.title}</h1>
      <SessionFilters orgId={org} query={query} />
      <SessionsTable orgId={org} sessions={page.items} />
      <Pagination orgId={org} query={query} nextCursor={page.next_cursor} shown={page.items.length} total={page.total} />
    </>
  );
}
