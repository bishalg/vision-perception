import type { Metadata } from "next";
import Link from "next/link";
import { authedApi, currentRole } from "@/lib/auth/server";
import { formatDateTime } from "@/lib/format";
import { getText } from "@/lib/i18n/server";
import { loadOrNotFound } from "@/lib/loaders";
import { can } from "@/lib/permissions";
import { describeCohort } from "@/lib/research/cohort";
import { pathsFor } from "@/lib/paths";


export async function generateMetadata(): Promise<Metadata> {
  const tx = await getText();
  const t = tx.research.cohorts;
  return { title: t.title };
}

const PAGE = 100;

/** `/[org]/research/cohorts`: the saved cohorts (RUI-5). */
export default async function CohortsPage({ params }: { params: Promise<{ org: string }> }) {
  const tx = await getText();
  const t = tx.research.cohorts;
  const { org } = await params;
  const role = await currentRole();
  const api = await authedApi();
  const page = await loadOrNotFound(() => api.listCohorts(org, { limit: PAGE }));
  const rp = pathsFor(org).research;
  return (
    <>
      <h1>{t.title}</h1>
      <p className="muted">{t.intro}</p>
      {can(role, "analysis.run") ? <p><Link href={rp.newCohort}>{t.newLink}</Link></p> : <p className="muted">{t.readOnly}</p>}
      {page.items.length === 0 ? (
        <p className="muted">{t.empty}</p>
      ) : (
        <div className="table-wrap">
          <table className="data" aria-label={t.title}>
            <thead>
              <tr><th scope="col">{t.cols.name}</th><th scope="col">{t.cols.definition}</th><th scope="col">{t.cols.created}</th><th scope="col">{t.cols.status}</th></tr>
            </thead>
            <tbody>
              {page.items.map((c) => (
                <tr key={c.id}>
                  <td><Link href={rp.cohort(c.id)}>{c.name}</Link></td>
                  <td>{describeCohort(c.definition).join("; ")}</td>
                  <td>{formatDateTime(c.created_at)}</td>
                  <td>{c.archived_at ? t.archived : t.active}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </>
  );
}
