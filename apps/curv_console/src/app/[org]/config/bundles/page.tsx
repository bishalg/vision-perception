import type { Metadata } from "next";
import Link from "next/link";
import { BundleCompose } from "@/components/config/BundleCompose";
import { CONFIG_DOCUMENT_KINDS, type ConfigDocumentKind } from "@/lib/api/catalog";
import { authedApi, currentRole } from "@/lib/auth/server";
import { formatDateTime } from "@/lib/format";
import { getText } from "@/lib/i18n/server";
import { parseCursor, toSearch } from "@/lib/listQuery";
import { loadOrNotFound } from "@/lib/loaders";
import { can } from "@/lib/permissions";
import { composeBundleAction } from "../docActions";


export async function generateMetadata(): Promise<Metadata> {
  const tx = await getText();
  const t = tx.config.bundles;
  return { title: t.title };
}

const SEGMENT: Record<ConfigDocumentKind, string> = {
  stimulus_catalog: "catalog",
  normative_baselines: "baselines",
  scoring_profile: "scoring",
};

/** `/[org]/config/bundles`: signed bundle history and "compose" from the published documents (spec 03 IA). */
export default async function BundlesPage({
  params,
  searchParams,
}: {
  params: Promise<{ org: string }>;
  searchParams: Promise<Record<string, string | string[] | undefined>>;
}) {
  const tx = await getText();
  const t = tx.config.bundles;
  const { org } = await params;
  const role = await currentRole();
  if (!can(role, "config.read")) {
    return (
      <>
        <h1>{t.title}</h1>
        <p className="muted">{tx.config.denied}</p>
      </>
    );
  }
  const cursor = parseCursor(await searchParams);
  const api = await authedApi();
  const [page, ...published] = await Promise.all([
    loadOrNotFound(() => api.listConfigBundles({ cursor, limit: 50 })),
    ...CONFIG_DOCUMENT_KINDS.map((k) =>
      loadOrNotFound(() => api.listConfigDocuments(k, { status: "published", limit: 1 })).then((p) => p.items[0]),
    ),
  ]);
  const o = `/${encodeURIComponent(org)}/config`;
  const ready = published.every(Boolean);

  return (
    <>
      <h1>{t.title}</h1>
      <p className="muted">{t.intro}</p>
      <section className="panel" aria-label={t.current}>
        <h2 style={{ marginTop: 0 }}>{t.current}</h2>
        <dl className="facts">
          {CONFIG_DOCUMENT_KINDS.map((k, i) => {
            const d = published[i];
            return (
              <div key={k}>
                <dt>{t.kinds[k]}</dt>
                <dd>{d ? <Link href={`${o}/${SEGMENT[k]}/${encodeURIComponent(d.version)}`}>{d.version}</Link> : t.missing}</dd>
              </div>
            );
          })}
        </dl>
      </section>
      {can(role, "config.publish") ? (
        <BundleCompose ready={ready} superadmin={role === "superadmin"} compose={composeBundleAction.bind(null, org)} />
      ) : (
        <p className="muted">{t.readOnly}</p>
      )}
      {page.items.length ? (
        <div className="table-wrap">
          <table className="data" aria-label={t.title}>
            <thead>
              <tr>
                <th>{t.cols.version}</th>
                <th>{t.cols.catalog}</th>
                <th>{t.cols.baselines}</th>
                <th>{t.cols.scoring}</th>
                <th>{t.cols.key}</th>
                <th>{t.cols.published}</th>
              </tr>
            </thead>
            <tbody>
              {page.items.map((b) => (
                <tr key={b.id}>
                  <td>{b.bundle_version}</td>
                  <td><Link href={`${o}/catalog/${encodeURIComponent(b.catalog_version)}`}>{b.catalog_version}</Link></td>
                  <td><Link href={`${o}/baselines/${encodeURIComponent(b.baselines_version)}`}>{b.baselines_version}</Link></td>
                  <td><Link href={`${o}/scoring/${encodeURIComponent(b.scoring_profile_version)}`}>{b.scoring_profile_version}</Link></td>
                  <td>{b.key_id}</td>
                  <td>{formatDateTime(b.published_at)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      ) : (
        <p className="muted">{t.empty}</p>
      )}
      <nav className="pagination" aria-label={tx.config.pagination}>
        {cursor ? <Link href={`${o}/bundles`}>{t.first}</Link> : null}
        {page.next_cursor ? <Link rel="next" href={`${o}/bundles${toSearch({ cursor: page.next_cursor })}`}>{t.next}</Link> : null}
      </nav>
    </>
  );
}
