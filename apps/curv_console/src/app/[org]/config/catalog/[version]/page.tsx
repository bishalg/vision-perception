import type { Metadata } from "next";
import Link from "next/link";
import { authorText } from "@/components/catalog/authorText";
import { CatalogDiffView, ConfigStatusBadge } from "@/components/catalog/CatalogTables";
import { CatalogViewer } from "@/components/catalog/CatalogViewer";
import { LifecycleActions } from "@/components/catalog/LifecycleActions";
import s from "@/components/catalog/catalog.module.css";
import { authedApi, currentRole } from "@/lib/auth/server";
import { diffCatalogs, parseCatalog } from "@/lib/catalog";
import { formatDateTime } from "@/lib/format";
import { getText } from "@/lib/i18n/server";
import { toSearch } from "@/lib/listQuery";
import { loadOrNotFound } from "@/lib/loaders";
import { can } from "@/lib/permissions";
import { transitionCatalogAction } from "../actions";


export async function generateMetadata(): Promise<Metadata> {
  const tx = await getText();
  const t = tx.catalog;
  return { title: t.title };
}

export default async function CatalogVersionPage({
  params,
  searchParams,
}: {
  params: Promise<{ org: string; version: string }>;
  searchParams: Promise<Record<string, string | string[] | undefined>>;
}) {
  const tx = await getText();
  const t = tx.catalog;
  const f = t.view.facts;
  const { org, version } = await params;
  const role = await currentRole();
  if (!can(role, "config.read")) {
    return (
      <>
        <h1>{t.title}</h1>
        <p className="muted">{t.denied}</p>
      </>
    );
  }
  const raw = await searchParams;
  const created = raw.created === "1";
  const compare = (Array.isArray(raw.compare) ? raw.compare[0] : raw.compare)?.slice(0, 64);
  const api = await authedApi();
  const [doc, history] = await Promise.all([
    loadOrNotFound(() => api.getConfigDocument("stimulus_catalog", version)),
    loadOrNotFound(() => api.listConfigDocuments("stimulus_catalog", { limit: 100 })),
  ]);
  const catalog = parseCatalog(doc.body);
  const other = compare && compare !== version
    ? await api.getConfigDocument("stimulus_catalog", compare).catch(() => null)
    : null;
  const otherCatalog = other ? parseCatalog(other.body) : null;
  const base = `/${encodeURIComponent(org)}/config/catalog`;
  const canManage = can(role, "config.manage");
  const others = history.items.filter((d) => d.version !== version);

  return (
    <>
      <p><Link href={base}>{t.back}</Link></p>
      <h1>
        {doc.version} <ConfigStatusBadge status={doc.status} />
      </h1>
      <dl className="facts panel">
        <div><dt>{f.status}</dt><dd>{t.statuses[doc.status]}</dd></div>
        <div><dt>{f.created}</dt><dd>{formatDateTime(doc.created_at)}</dd></div>
        <div><dt>{f.author}</dt><dd title={doc.created_by ?? undefined}>{authorText(doc.created_by, t.system)}</dd></div>
        <div><dt>{f.published}</dt><dd>{doc.published_at ? formatDateTime(doc.published_at) : "—"}</dd></div>
        {catalog ? (
          <>
            <div><dt>{f.clips}</dt><dd>{catalog.clips.length}</dd></div>
            <div><dt>{f.schema}</dt><dd>{catalog.schemaVersion}</dd></div>
            <div><dt>{f.languages}</dt><dd>{catalog.languages.join(", ")} ({catalog.fallbackLanguage})</dd></div>
            <div><dt>{f.agePacks}</dt><dd>{catalog.agePacks.map((p) => p.id).join(", ") || "—"}</dd></div>
          </>
        ) : null}
      </dl>
      {created ? <p role="status" className={s.ok}>{t.view.created}</p> : null}
      {doc.notes ? <p><strong>{t.view.notes}:</strong> {doc.notes}</p> : null}
      {catalog?.comment ? <p className="muted"><strong>{t.view.comment}:</strong> {catalog.comment}</p> : null}

      {canManage ? (
        <div className={s.section}>
          <LifecycleActions status={doc.status} run={transitionCatalogAction.bind(null, org, doc.version)} />
          {catalog ? (
            <p>
              <Link className="button" href={`${base}/new${toSearch({ from: doc.version })}`}>{t.view.editAsDraft}</Link>
            </p>
          ) : null}
        </div>
      ) : (
        <p className="muted">{t.readOnly}</p>
      )}

      {others.length ? (
        <form method="get" className={`${s.row} ${s.section}`} aria-label={t.view.compare}>
          <label>
            {t.view.compare}
            <select name="compare" defaultValue={other?.version ?? ""}>
              <option value="">{t.view.compareNone}</option>
              {others.map((d) => (
                <option key={d.id} value={d.version}>{d.version} ({t.statuses[d.status]})</option>
              ))}
            </select>
          </label>
          <button type="submit" className="secondary">{t.view.compareSubmit}</button>
        </form>
      ) : null}
      {catalog && other && otherCatalog ? (
        <div className={s.section}>
          <CatalogDiffView from={other.version} to={doc.version} diff={diffCatalogs(otherCatalog, catalog)} />
        </div>
      ) : null}

      {catalog ? (
        <CatalogViewer catalog={catalog} />
      ) : (
        <>
          <p className="muted">{t.view.unreadable}</p>
          <pre className={s.pre}>{JSON.stringify(doc.body, null, 2)}</pre>
        </>
      )}
    </>
  );
}
