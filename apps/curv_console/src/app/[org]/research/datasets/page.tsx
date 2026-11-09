import type { Metadata } from "next";
import Link from "next/link";
import { authedApi, currentRole } from "@/lib/auth/server";
import { formatDateTime } from "@/lib/format";
import { getText } from "@/lib/i18n/server";
import { loadOrNotFound } from "@/lib/loaders";
import { pathsFor } from "@/lib/paths";
import { can } from "@/lib/permissions";
import { shortHash } from "@/lib/research/datasets";
import { enumLabel } from "@/messages/domain";

export async function generateMetadata(): Promise<Metadata> {
  return { title: (await getText()).research.datasets.title };
}

const PAGE = 100;

/** `/[org]/research/datasets`: the org's frozen datasets, newest first (WP-X.11). */
export default async function DatasetsPage({ params }: { params: Promise<{ org: string }> }) {
  const tx = await getText();
  const t = tx.research.datasets;
  const { org } = await params;
  const role = await currentRole();
  const api = await authedApi();
  const page = await loadOrNotFound(() => api.listDatasets(org, { limit: PAGE }));
  const rp = pathsFor(org).research;
  return (
    <>
      <h1>{t.title}</h1>
      <p className="muted">{t.intro}</p>
      {can(role, "experiment.write") ? <p><Link href={rp.newDataset}>{t.newLink}</Link></p> : <p className="muted">{t.readOnly}</p>}
      {page.items.length === 0 ? (
        <p className="muted">{t.empty}</p>
      ) : (
        <div className="table-wrap">
          <table className="data" aria-label={t.title}>
            <thead>
              <tr>
                <th scope="col">{t.cols.name}</th>
                <th scope="col">{t.cols.members}</th>
                <th scope="col">{t.cols.hash}</th>
                <th scope="col">{t.cols.frozen}</th>
                <th scope="col">{t.cols.status}</th>
              </tr>
            </thead>
            <tbody>
              {page.items.map((d) => (
                <tr key={d.id}>
                  <td><Link href={rp.dataset(d.id)}>{d.name}</Link></td>
                  <td>{d.member_count}</td>
                  <td><code title={d.content_hash}>{shortHash(d.content_hash)}</code></td>
                  <td>{formatDateTime(d.frozen_at)}</td>
                  <td>{enumLabel(tx.locale, "dataset_status", d.status)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </>
  );
}
