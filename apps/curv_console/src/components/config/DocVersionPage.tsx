/**
 * One version of a baselines or scoring-profile document (WP-C.5): facts, changelog, lifecycle with the sandbox gate,
 * "edit as new draft", compare with another version, and the read-only view. Server component.
 */
import Link from "next/link";
import { transitionDocAction, type EditableKind } from "@/app/[org]/config/docActions";
import { authorText } from "@/components/catalog/authorText";
import { ConfigStatusBadge } from "@/components/catalog/CatalogTables";
import s from "@/components/catalog/catalog.module.css";
import { authedApi, currentRole } from "@/lib/auth/server";
import { diffDocuments, parseBaselines, parseProfile } from "@/lib/configDocs";
import { formatDateTime } from "@/lib/format";
import { getText } from "@/lib/i18n/server";
import { toSearch } from "@/lib/listQuery";
import { loadOrNotFound } from "@/lib/loaders";
import { can } from "@/lib/permissions";
import { DocLifecycle } from "./DocLifecycle";
import { BaselinesView, DocDiffView, ScoringProfileView } from "./DocViews";

export async function DocVersionPage({
  org,
  kind,
  version,
  searchParams,
}: {
  org: string;
  kind: EditableKind;
  version: string;
  searchParams: Record<string, string | string[] | undefined>;
}) {
  const tx = await getText();
  const t = tx.config;
  const isProfile = kind === "scoring_profile";
  const text = isProfile ? t.scoring : t.baselines;
  const segment = isProfile ? "scoring" : "baselines";
  const role = await currentRole();
  if (!can(role, "config.read")) {
    return (
      <>
        <h1>{text.title}</h1>
        <p className="muted">{t.denied}</p>
      </>
    );
  }
  const created = searchParams.created === "1";
  const compare = (Array.isArray(searchParams.compare) ? searchParams.compare[0] : searchParams.compare)?.slice(0, 64);
  const api = await authedApi();
  const [doc, history] = await Promise.all([
    loadOrNotFound(() => api.getConfigDocument(kind, version)),
    loadOrNotFound(() => api.listConfigDocuments(kind, { limit: 100 })),
  ]);
  const other = compare && compare !== version ? await api.getConfigDocument(kind, compare).catch(() => null) : null;
  const profile = isProfile ? parseProfile(doc.body) : null;
  const set = isProfile ? null : parseBaselines(doc.body);
  const readable = Boolean(profile ?? set);
  const base = `/${encodeURIComponent(org)}/config/${segment}`;
  const sandboxHref = `/${encodeURIComponent(org)}/config/sandbox${toSearch(isProfile ? { profile: doc.version } : { baselines: doc.version })}`;
  const others = history.items.filter((d) => d.version !== version);
  const f = t.facts;

  return (
    <>
      <p><Link href={base}>{t.back}</Link></p>
      <h1>
        {text.title} {doc.version} <ConfigStatusBadge status={doc.status} />
      </h1>
      <dl className="facts panel">
        <div><dt>{f.status}</dt><dd>{tx.catalog.statuses[doc.status]}</dd></div>
        <div><dt>{f.created}</dt><dd>{formatDateTime(doc.created_at)}</dd></div>
        <div><dt>{f.author}</dt><dd title={doc.created_by ?? undefined}>{authorText(doc.created_by, tx.catalog.system)}</dd></div>
        <div><dt>{f.published}</dt><dd>{doc.published_at ? formatDateTime(doc.published_at) : "—"}</dd></div>
        {set ? (
          <>
            <div><dt>{t.baselines.facts.videos}</dt><dd>{Object.keys(set.videos).length}</dd></div>
            <div><dt>{t.baselines.facts.brackets}</dt><dd>{set.ageBrackets.map((x) => x.id).join(", ") || "—"}</dd></div>
            <div><dt>{t.baselines.facts.status}</dt><dd>{set.status}</dd></div>
            <div><dt>{t.baselines.facts.schema}</dt><dd>{set.schemaVersion}</dd></div>
          </>
        ) : null}
      </dl>
      {created ? <p role="status" className={s.ok}>{t.created}</p> : null}
      {doc.notes ? <p><strong>{f.notes}:</strong> {doc.notes}</p> : null}
      {set?.provenance ? <p className="muted"><strong>{t.baselines.facts.provenance}:</strong> {set.provenance}</p> : null}

      {can(role, "config.manage") ? (
        <div className={s.section}>
          <DocLifecycle
            kind={kind}
            version={doc.version}
            status={doc.status}
            hints={text.lifecycle}
            sandboxHref={sandboxHref}
            run={transitionDocAction.bind(null, org, kind, doc.version)}
          />
          {readable ? (
            <p>
              <Link className="button" href={`${base}/new${toSearch({ from: doc.version })}`}>{t.editAsDraft}</Link>
            </p>
          ) : null}
        </div>
      ) : (
        <>
          <p className="muted">{t.readOnly}</p>
          <p><Link href={sandboxHref}>{t.runSandbox}</Link></p>
        </>
      )}

      {others.length ? (
        <form method="get" className={`${s.row} ${s.section}`} aria-label={t.compare}>
          <label>
            {t.compare}
            <select name="compare" defaultValue={other?.version ?? ""}>
              <option value="">{t.compareNone}</option>
              {others.map((d) => (
                <option key={d.id} value={d.version}>{d.version} ({tx.catalog.statuses[d.status]})</option>
              ))}
            </select>
          </label>
          <button type="submit" className="secondary">{t.compareSubmit}</button>
        </form>
      ) : null}
      {other ? (
        <div className={s.section}>
          <DocDiffView from={other.version} to={doc.version} changes={diffDocuments(other.body, doc.body)} groupByClip={!isProfile} />
        </div>
      ) : null}

      <div className={s.section}>
        {profile ? <ScoringProfileView profile={profile} /> : null}
        {set ? <BaselinesView set={set} /> : null}
        {!readable ? (
          <>
            <p className="muted">{t.unreadable}</p>
            <pre className={s.pre}>{JSON.stringify(doc.body, null, 2)}</pre>
          </>
        ) : null}
      </div>
    </>
  );
}
