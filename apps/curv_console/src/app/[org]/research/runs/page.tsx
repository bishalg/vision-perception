import type { Metadata } from "next";
import Link from "next/link";
import { RUN_STATUSES, type RunStatus } from "@/lib/api/research";
import { authedApi } from "@/lib/auth/server";
import { formatDateTime } from "@/lib/format";
import { parseCursor, toSearch } from "@/lib/listQuery";
import { loadOrNotFound } from "@/lib/loaders";
import { researchText } from "@/messages/research";

const t = researchText.runs;

export const metadata: Metadata = { title: t.title };

const PAGE = 25;

/** `/[org]/research/runs`: the analysis runs, newest first, optionally filtered by status (RUI-6). */
export default async function RunsPage({
  params,
  searchParams,
}: {
  params: Promise<{ org: string }>;
  searchParams: Promise<Record<string, string | string[] | undefined>>;
}) {
  const { org } = await params;
  const query = await searchParams;
  const rawStatus = Array.isArray(query.status) ? query.status[0] : query.status;
  const status = (RUN_STATUSES as readonly string[]).includes(rawStatus ?? "") ? (rawStatus as RunStatus) : undefined;
  const cursor = parseCursor(query);
  const api = await authedApi();
  const [page, experiments, cohorts] = await Promise.all([
    loadOrNotFound(() => api.listAnalysisRuns(org, { limit: PAGE, ...(status ? { status } : {}), ...(cursor ? { cursor } : {}) })),
    api.listExperiments(org, { limit: 100 }).then((p) => new Map(p.items.map((e) => [e.id, e.name]))).catch(() => new Map<string, string>()),
    api.listCohorts(org, { limit: 100 }).then((p) => new Map(p.items.map((c) => [c.id, c.name]))).catch(() => new Map<string, string>()),
  ]);
  const base = `/${encodeURIComponent(org)}/research/runs`;
  return (
    <>
      <h1>{t.title}</h1>
      <p className="muted">{t.intro}</p>
      <form method="get" className="filter-actions" aria-label={t.filterStatus}>
        <label>
          {t.filterStatus}
          <select name="status" defaultValue={status ?? ""}>
            <option value="">{t.allStatuses}</option>
            {RUN_STATUSES.map((s) => <option key={s} value={s}>{researchText.runStatus[s]}</option>)}
          </select>
        </label>
        <button type="submit" className="secondary">{t.apply}</button>
      </form>
      {page.items.length === 0 ? (
        <p className="muted">{t.empty}</p>
      ) : (
        <div className="table-wrap">
          <table className="data" aria-label={t.title}>
            <thead>
              <tr>
                <th scope="col">{t.cols.status}</th><th scope="col">{t.cols.kind}</th><th scope="col">{t.cols.experiment}</th>
                <th scope="col">{t.cols.cohort}</th><th scope="col" className="num">{t.cols.sessions}</th><th scope="col">{t.cols.created}</th>
              </tr>
            </thead>
            <tbody>
              {page.items.map((run) => (
                <tr key={run.id}>
                  <td><Link href={`${base}/${encodeURIComponent(run.id)}`}>{researchText.runStatus[run.status]}</Link></td>
                  <td>{t.kind[run.kind]}</td>
                  <td>{(run.experiment_id && experiments.get(run.experiment_id)) || t.unknownExperiment}</td>
                  <td>{(run.cohort_id && cohorts.get(run.cohort_id)) || t.unknownCohort}</td>
                  <td className="num">{run.sessions_n ?? researchText.charts.noTransition}</td>
                  <td>{formatDateTime(run.created_at)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
      <nav className="pagination" aria-label={t.title}>
        {cursor ? <Link href={`${base}${toSearch({ ...(status ? { status } : {}) })}`}>{t.first}</Link> : null}
        {page.next_cursor ? <Link rel="next" href={`${base}${toSearch({ ...(status ? { status } : {}), cursor: page.next_cursor })}`}>{t.next}</Link> : null}
      </nav>
    </>
  );
}
