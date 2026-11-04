import type { Metadata } from "next";
import Link from "next/link";
import { notFound } from "next/navigation";
import { RunSessionsTable } from "@/components/research/RunSessionsTable";
import { authedApi } from "@/lib/auth/server";
import { getText } from "@/lib/i18n/server";
import { parseCursor, toSearch } from "@/lib/listQuery";
import { loadOrNotFound } from "@/lib/loaders";


export async function generateMetadata(): Promise<Metadata> {
  const tx = await getText();
  const t = tx.research.runs;
  return { title: t.sessionsTitle };
}

const PAGE = 50;

/** `/[org]/research/runs/[id]/sessions`: the per-session rows of a run (RUN-3), paged by cursor. */
export default async function RunSessionsPage({
  params,
  searchParams,
}: {
  params: Promise<{ org: string; id: string }>;
  searchParams: Promise<Record<string, string | string[] | undefined>>;
}) {
  const tx = await getText();
  const t = tx.research.runs;
  const { org, id } = await params;
  const cursor = parseCursor(await searchParams);
  const api = await authedApi();
  const run = await loadOrNotFound(() => api.getAnalysisRun(id));
  if (run.org_id !== org) notFound();
  const page = await loadOrNotFound(() => api.listAnalysisRunSessions(id, { limit: PAGE, ...(cursor ? { cursor } : {}) }));
  const base = `/${encodeURIComponent(org)}/research/runs/${encodeURIComponent(id)}`;
  return (
    <>
      <p><Link href={base}>{t.backToRun}</Link></p>
      <h1>{t.sessionsTitle}</h1>
      <RunSessionsTable rows={page.items} text={tx.research} />
      <nav className="pagination" aria-label={t.sessionsTitle}>
        {cursor ? <Link href={`${base}/sessions`}>{t.first}</Link> : null}
        {page.next_cursor ? <Link rel="next" href={`${base}/sessions${toSearch({ cursor: page.next_cursor })}`}>{t.next}</Link> : null}
      </nav>
    </>
  );
}
