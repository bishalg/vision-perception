import type { Metadata } from "next";
import Link from "next/link";
import { ParticipantsTable } from "@/components/ParticipantsTable";
import { authedApi } from "@/lib/auth/server";
import { parseCursor, toSearch } from "@/lib/listQuery";
import { loadOrNotFound } from "@/lib/loaders";
import { adminText } from "@/messages/admin";

const t = adminText.participants;

export const metadata: Metadata = { title: t.title };

export default async function ParticipantsPage({
  params,
  searchParams,
}: {
  params: Promise<{ org: string }>;
  searchParams: Promise<Record<string, string | string[] | undefined>>;
}) {
  const { org } = await params;
  const cursor = parseCursor(await searchParams);
  const api = await authedApi();
  const page = await loadOrNotFound(() => api.listParticipants(org, { cursor, limit: 50 }));
  const base = `/${encodeURIComponent(org)}/participants`;
  return (
    <>
      <h1>{t.title}</h1>
      <p className="muted">{t.intro}</p>
      <ParticipantsTable participants={page.items} />
      <p className="muted">{t.showing(page.items.length, page.total)}</p>
      <nav className="pagination" aria-label="Pagination">
        {cursor ? <Link href={base}>{adminText.common.first}</Link> : null}
        {page.next_cursor ? (
          <Link rel="next" href={`${base}${toSearch({ cursor: page.next_cursor })}`}>{adminText.common.next}</Link>
        ) : null}
      </nav>
    </>
  );
}
