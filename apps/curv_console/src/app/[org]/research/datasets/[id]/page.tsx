import type { Metadata } from "next";
import Link from "next/link";
import { notFound } from "next/navigation";
import { ArchiveDatasetButton } from "@/components/research/ArchiveDatasetButton";
import { DatasetHash } from "@/components/research/DatasetHash";
import { DatasetRunStart } from "@/components/research/DatasetRunStart";
import { SplitSummary } from "@/components/research/SplitSummary";
import type { Lineage } from "@/lib/api/research";
import { authedApi, currentRole } from "@/lib/auth/server";
import { formatDateTime } from "@/lib/format";
import { getText } from "@/lib/i18n/server";
import { loadOrNotFound } from "@/lib/loaders";
import { pathsFor } from "@/lib/paths";
import { can } from "@/lib/permissions";
import { enumLabel } from "@/messages/domain";
import { archiveDatasetAction } from "../actions";

export async function generateMetadata(): Promise<Metadata> {
  return { title: (await getText()).research.datasets.title };
}

const MEMBERS_PAGE = 50;
const LINEAGE_FIELDS = [
  "cohort_hash",
  "parent_dataset_id",
  "run_id",
  "profile_hash",
  "profile_version",
  "catalog_version",
  "baselines_version",
  "code_version",
] as const satisfies readonly (keyof Lineage)[];

/**
 * `/[org]/research/datasets/[id]` (WP-X.11, R-7..R-9): facts and content hash, lineage, the data dictionary, the split
 * and labels, the members (paged; each page read is audited), the exclusions with their rule and reason, "Start run
 * from this dataset" and the export pre-fill. Everything shown is the backend's frozen record.
 */
export default async function DatasetPage({
  params,
  searchParams,
}: {
  params: Promise<{ org: string; id: string }>;
  searchParams: Promise<Record<string, string | string[] | undefined>>;
}) {
  const tx = await getText();
  const t = tx.research.datasets;
  const { org, id } = await params;
  const query = await searchParams;
  const rawCursor = Array.isArray(query.cursor) ? query.cursor[0] : query.cursor;
  const cursor = rawCursor && rawCursor.length <= 500 ? rawCursor : undefined;
  const role = await currentRole();
  const api = await authedApi();
  const dataset = await loadOrNotFound(() => api.getDataset(id));
  if (dataset.org_id !== org) notFound();
  const [members, experiments] = await Promise.all([
    loadOrNotFound(() => api.listDatasetMembers(id, { limit: MEMBERS_PAGE, ...(cursor ? { cursor } : {}) })),
    can(role, "analysis.run") && dataset.status === "frozen"
      ? api.listExperiments(org, { status: "draft", limit: 100 }).then((p) => p.items).catch(() => [])
      : Promise.resolve([]),
  ]);
  const p = pathsFor(org);
  const rp = p.research;
  const loc = tx.locale;
  const valueOrNone = (v: string | null | undefined) => (v == null || v === "" ? <span className="muted">{t.none}</span> : <code>{v}</code>);
  return (
    <>
      <p><Link href={rp.datasets}>{t.allDatasets}</Link></p>
      <h1>{dataset.name}</h1>
      {dataset.description ? <p>{dataset.description}</p> : null}
      {dataset.status === "archived" ? <p className="muted">{t.archivedNote}</p> : null}

      <section className="panel stack" aria-label={t.facts}>
        <h2>{t.facts}</h2>
        <dl>
          <dt>{t.hash.label}</dt>
          <dd><DatasetHash hash={dataset.content_hash} /></dd>
          <dt>{t.frozenAt}</dt>
          <dd>{formatDateTime(dataset.frozen_at)}</dd>
          <dt>{t.cols.status}</dt>
          <dd>{enumLabel(loc, "dataset_status", dataset.status)}</dd>
          <dt>{t.cohort}</dt>
          <dd>{dataset.cohort_id ? <Link href={rp.cohort(dataset.cohort_id)}>{dataset.cohort_id}</Link> : <span className="muted">{t.none}</span>}</dd>
          <dt>{t.cols.members}</dt>
          <dd>
            {t.memberCount(dataset.member_count)}
            {dataset.erased_count ? <> <span className="muted">({t.erasedCount(dataset.erased_count)})</span></> : null}
          </dd>
        </dl>
      </section>

      <section className="panel stack" aria-label={t.lineage}>
        <h2>{t.lineage}</h2>
        <dl>
          {LINEAGE_FIELDS.map((f) => (
            <div key={f} style={{ display: "contents" }} data-lineage={f}>
              <dt>{t.lineageFields[f]}</dt>
              <dd>
                {f === "parent_dataset_id" && dataset.lineage[f]
                  ? <Link href={rp.dataset(dataset.lineage[f]!)}>{dataset.lineage[f]}</Link>
                  : f === "run_id" && dataset.lineage[f]
                    ? <Link href={rp.run(dataset.lineage[f]!)}>{dataset.lineage[f]}</Link>
                    : valueOrNone(dataset.lineage[f])}
              </dd>
            </div>
          ))}
        </dl>
      </section>

      <section className="panel stack" aria-label={t.splitTitle}>
        <h2>{t.splitTitle}</h2>
        <SplitSummary dataset={dataset} text={tx.research} />
      </section>

      <section className="panel stack" aria-label={t.labelsTitle}>
        <h2>{t.labelsTitle}</h2>
        {dataset.label_source ? (
          <p>{t.labelSource(enumLabel(loc, "label_source_kind", dataset.label_source.kind), dataset.label_source.count, formatDateTime(dataset.label_source.taken_at))}</p>
        ) : (
          <p className="muted">{t.noLabels}</p>
        )}
      </section>

      <section className="panel stack" aria-label={t.members}>
        <h2>{t.members}</h2>
        <div className="table-wrap">
          <table className="data" aria-label={t.members}>
            <thead>
              <tr>
                <th scope="col">{t.memberCols.session}</th>
                <th scope="col">{t.memberCols.pseudonym}</th>
                <th scope="col">{t.memberCols.age}</th>
                <th scope="col">{t.memberCols.band}</th>
                <th scope="col">{t.memberCols.split}</th>
                <th scope="col">{t.memberCols.label}</th>
              </tr>
            </thead>
            <tbody>
              {members.items.map((m) =>
                m.excluded ? (
                  <tr key={m.session_id} data-member={m.session_id}>
                    <td><code>{m.session_id}</code></td>
                    <td colSpan={5} className="muted">{t.erasedMember}</td>
                  </tr>
                ) : (
                  <tr key={m.session_id} data-member={m.session_id}>
                    <td><code>{m.session_id}</code></td>
                    <td>{m.participant_pseudonym ?? ""}</td>
                    <td>{m.age_months ?? ""}</td>
                    <td>{m.age_band ?? ""}</td>
                    <td>{m.split ? enumLabel(loc, "dataset_split", m.split) : ""}</td>
                    <td>{m.label ? `${enumLabel(loc, "disposition", m.label.value)} (${m.label.reviewer_pseudonym ?? ""})` : ""}</td>
                  </tr>
                ),
              )}
            </tbody>
          </table>
        </div>
        <nav className="pagination" aria-label={t.members}>
          {cursor ? <Link href={rp.dataset(id)}>{t.firstPage}</Link> : null}
          {members.next_cursor ? <Link rel="next" href={`${rp.dataset(id)}?cursor=${encodeURIComponent(members.next_cursor)}`}>{t.nextPage}</Link> : null}
        </nav>
      </section>

      <section className="panel stack" aria-label={t.exclusions}>
        <h2>{t.exclusions}</h2>
        <h3>{t.rules}</h3>
        {dataset.exclusion_rules.length === 0 ? (
          <p className="muted">{t.noRules}</p>
        ) : (
          <ul>
            {dataset.exclusion_rules.map((rule) => (
              <li key={rule.id}><code>{rule.id}</code>: {enumLabel(loc, "exclusion_rule_kind", rule.kind)}</li>
            ))}
          </ul>
        )}
        {dataset.exclusions.length === 0 ? (
          <p className="muted">{t.noExclusions}</p>
        ) : (
          <div className="table-wrap">
            <table className="data" aria-label={t.exclusions}>
              <thead>
                <tr>
                  <th scope="col">{t.exclusionCols.session}</th>
                  <th scope="col">{t.exclusionCols.rule}</th>
                  <th scope="col">{t.exclusionCols.reason}</th>
                </tr>
              </thead>
              <tbody>
                {dataset.exclusions.map((e) => (
                  <tr key={e.session_id} data-exclusion={e.session_id}>
                    <td><code>{e.session_id}</code></td>
                    <td>{e.rule_id ? <code>{e.rule_id}</code> : <span className="muted">{t.systemRule}</span>}</td>
                    <td>{t.reasons[e.reason] ?? e.reason}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </section>

      <section className="panel stack" aria-label={t.dictionary}>
        <h2>{t.dictionary}</h2>
        <div className="table-wrap">
          <table className="data" aria-label={t.dictionary}>
            <thead>
              <tr>
                <th scope="col">{t.dictCols.name}</th>
                <th scope="col">{t.dictCols.type}</th>
                <th scope="col">{t.dictCols.unit}</th>
                <th scope="col">{t.dictCols.description}</th>
              </tr>
            </thead>
            <tbody>
              {dataset.data_dictionary.map((c) => (
                <tr key={c.name}>
                  <td><code>{c.name}</code></td>
                  <td><code>{c.type}</code></td>
                  <td>{c.unit ? enumLabel(loc, "unit", c.unit) : ""}</td>
                  <td>{enumLabel(loc, "dataset_column", c.name)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </section>

      {can(role, "analysis.run") && dataset.status === "frozen" ? (
        <section className="panel stack" aria-label={t.runTitle}>
          <h2>{t.runTitle}</h2>
          <p className="muted">{t.runIntro}</p>
          <DatasetRunStart orgId={org} datasetId={id} experiments={experiments.map((e) => ({ id: e.id, name: e.name }))} />
        </section>
      ) : null}

      {can(role, "export.create") ? (
        <p>
          <Link href={p.data.exportDataset(id)}>{t.exportLink}</Link> <span className="muted">{t.exportHint}</span>
        </p>
      ) : null}

      {can(role, "experiment.write") && dataset.status === "frozen" ? (
        <ArchiveDatasetButton archive={archiveDatasetAction.bind(null, org, id)} />
      ) : null}
    </>
  );
}
