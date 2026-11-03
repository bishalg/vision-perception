import type { Metadata } from "next";
import Link from "next/link";
import { authedApi, currentRole } from "@/lib/auth/server";
import { formatDateTime } from "@/lib/format";
import { loadOrNotFound } from "@/lib/loaders";
import { can } from "@/lib/permissions";
import { researchText } from "@/messages/research";

const t = researchText.experiments;

export const metadata: Metadata = { title: t.title };

const PAGE = 50;

export default async function ExperimentsPage({ params }: { params: Promise<{ org: string }> }) {
  const { org } = await params;
  const role = await currentRole();
  const api = await authedApi();
  const page = await loadOrNotFound(() => api.listExperiments(org, { limit: PAGE }));
  const base = `/${encodeURIComponent(org)}/research/experiments`;
  const canWrite = can(role, "experiment.write");
  return (
    <>
      <h1>{t.title}</h1>
      <p className="muted">{t.intro}</p>
      {canWrite ? <p><Link href={`${base}/new`}>{t.newLink}</Link></p> : <p className="muted">{t.readOnly}</p>}
      {page.items.length === 0 ? (
        <p className="muted">{t.empty}</p>
      ) : (
        <div className="table-wrap">
          <table className="data" aria-label={t.title}>
            <thead>
              <tr><th>{t.cols.name}</th><th>{t.cols.status}</th><th>{t.cols.base}</th><th>{t.cols.updated}</th><th>{t.cols.lastRun}</th></tr>
            </thead>
            <tbody>
              {page.items.map((e) => (
                <tr key={e.id}>
                  <td><Link href={`${base}/${encodeURIComponent(e.id)}`}>{e.name}</Link></td>
                  <td>{t.status[e.status]}</td>
                  <td>{e.base_profile_version}</td>
                  <td>{formatDateTime(e.updated_at)}</td>
                  <td>{e.last_run_id ?? "-"}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </>
  );
}
