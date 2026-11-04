import type { Metadata } from "next";
import Link from "next/link";
import { WorklistFilters, WorklistTable } from "@/components/clinical/WorklistTable";
import { authedApi, currentRole } from "@/lib/auth/server";
import { toSearch } from "@/lib/listQuery";
import { loadOrNotFound } from "@/lib/loaders";
import { can } from "@/lib/permissions";
import { parseWorklistQuery } from "@/lib/worklistQuery";
import { getText } from "@/lib/i18n/server";

// A fixed title: nothing from the data (participant, MRN, impression) may reach the document title (UI-4).
export async function generateMetadata(): Promise<Metadata> {
  return { title: (await getText()).clinical.worklist.title };
}

type Raw = Record<string, string | string[] | undefined>;

export default async function WorklistPage({
  params,
  searchParams,
}: {
  params: Promise<{ org: string }>;
  searchParams: Promise<Raw>;
}) {
  const { org } = await params;
  const t = (await getText()).clinical.worklist;
  const role = await currentRole();
  if (!can(role, "worklist.read")) {
    return (
      <>
        <h1>{t.title}</h1>
        <p className="muted">{t.denied}</p>
      </>
    );
  }
  const query = parseWorklistQuery(await searchParams);
  const api = await authedApi();
  const page = await loadOrNotFound(() => api.getWorklist(org, { ...query, limit: 50 }));
  const base = `/${encodeURIComponent(org)}/worklist`;
  const keep = { state: query.state, flag: query.flag, assigned: query.assigned };
  return (
    <>
      <h1>{t.title}</h1>
      <p className="muted">{t.intro}</p>
      <WorklistFilters state={query.state} flag={query.flag} assigned={query.assigned} />
      <WorklistTable orgId={org} items={page.items} />
      <p className="muted">{t.showing(page.items.length, page.total)}</p>
      <nav className="pagination" aria-label={t.pagination}>
        {query.cursor ? <Link href={`${base}${toSearch(keep)}`}>{t.first}</Link> : null}
        {page.next_cursor ? (
          <Link rel="next" href={`${base}${toSearch({ ...keep, cursor: page.next_cursor })}`}>
            {t.next}
          </Link>
        ) : null}
      </nav>
    </>
  );
}
