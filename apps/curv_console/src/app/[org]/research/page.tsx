import type { Metadata } from "next";
import Link from "next/link";
import { authedApi } from "@/lib/auth/server";
import { formatDateTime } from "@/lib/format";
import { getText } from "@/lib/i18n/server";
import { loadOrNotFound } from "@/lib/loaders";


export async function generateMetadata(): Promise<Metadata> {
  const tx = await getText();
  const t = tx.research;
  return { title: t.title };
}

const RECENT = 5;

/** `/[org]/research`: recent experiments, cohorts and runs (RUI-1). Reads only. */
export default async function ResearchOverviewPage({ params }: { params: Promise<{ org: string }> }) {
  const tx = await getText();
  const t = tx.research;
  const { org } = await params;
  const api = await authedApi();
  const [experiments, cohorts, runs] = await Promise.all([
    loadOrNotFound(() => api.listExperiments(org, { limit: RECENT })),
    loadOrNotFound(() => api.listCohorts(org, { limit: RECENT })),
    loadOrNotFound(() => api.listAnalysisRuns(org, { limit: RECENT })),
  ]);
  const base = `/${encodeURIComponent(org)}/research`;
  return (
    <>
      <h1>{t.title}</h1>
      <div className="stack">
        <section className="panel" aria-label={t.experiments.title}>
          <h2>{t.experiments.title}</h2>
          {experiments.items.length === 0 ? <p className="muted">{t.experiments.empty}</p> : (
            <ul>
              {experiments.items.map((e) => (
                <li key={e.id}><Link href={`${base}/experiments/${encodeURIComponent(e.id)}`}>{e.name}</Link> <span className="muted">{t.experiments.status[e.status]}</span></li>
              ))}
            </ul>
          )}
          <p><Link href={`${base}/experiments`}>{t.experiments.allExperiments}</Link></p>
        </section>
        <section className="panel" aria-label={t.experiments.cohorts}>
          <h2>{t.experiments.cohorts}</h2>
          {cohorts.items.length === 0 ? <p className="muted">{t.experiments.noCohorts}</p> : (
            <ul>{cohorts.items.map((c) => <li key={c.id}><Link href={`${base}/cohorts/${encodeURIComponent(c.id)}`}>{c.name}</Link></li>)}</ul>
          )}
        </section>
        <section className="panel" aria-label={t.experiments.recentRuns}>
          <h2>{t.experiments.recentRuns}</h2>
          {runs.items.length === 0 ? <p className="muted">{t.experiments.noRuns}</p> : (
            <ul>
              {runs.items.map((x) => (
                <li key={x.id}><Link href={`${base}/runs/${encodeURIComponent(x.id)}`}>{t.runStatus[x.status]}</Link> <span className="muted">{formatDateTime(x.created_at)}</span></li>
              ))}
            </ul>
          )}
        </section>
      </div>
    </>
  );
}
