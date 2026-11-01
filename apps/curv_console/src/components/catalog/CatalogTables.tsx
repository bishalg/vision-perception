import Link from "next/link";
import type { ConfigDocument, ConfigDocumentStatus } from "@/lib/api/catalog";
import type { CatalogDiff } from "@/lib/catalog";
import { formatDateTime } from "@/lib/format";
import { catalogText } from "@/messages/catalog";
import s from "./catalog.module.css";

const t = catalogText;

const STATUS_BADGE: Record<ConfigDocumentStatus, string> = {
  draft: "badge-yellow",
  candidate: "badge-yellow",
  published: "badge-green",
  retired: "badge-unreliable",
};

export function ConfigStatusBadge({ status }: { status: ConfigDocumentStatus }) {
  return <span className={`badge ${STATUS_BADGE[status]}`}>{t.statuses[status]}</span>;
}

/** Author column: user ids are opaque; a null author is the system seed. */
export function authorText(createdBy: string | null | undefined): string {
  return createdBy ? createdBy.slice(0, 8) : t.system;
}

export function VersionsTable({ orgId, items }: { orgId: string; items: ConfigDocument[] }) {
  const base = `/${encodeURIComponent(orgId)}/config/catalog`;
  if (!items.length) return <p className="muted">{t.empty}</p>;
  return (
    <div className="table-wrap">
      <table className="data">
        <thead>
          <tr>
            <th>{t.cols.version}</th>
            <th>{t.cols.status}</th>
            <th>{t.cols.created}</th>
            <th>{t.cols.author}</th>
            <th>{t.cols.published}</th>
            <th>{t.cols.notes}</th>
          </tr>
        </thead>
        <tbody>
          {items.map((d) => (
            <tr key={d.id}>
              <td><Link href={`${base}/${encodeURIComponent(d.version)}`}>{d.version}</Link></td>
              <td><ConfigStatusBadge status={d.status} /></td>
              <td>{formatDateTime(d.created_at)}</td>
              <td title={d.created_by ?? undefined}>{authorText(d.created_by)}</td>
              <td>{d.published_at ? formatDateTime(d.published_at) : "—"}</td>
              <td style={{ whiteSpace: "normal" }}>{d.notes ?? ""}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

export function CatalogDiffView({ from, to, diff }: { from: string; to: string; diff: CatalogDiff }) {
  const empty = !diff.fields.length && !diff.added.length && !diff.removed.length && !diff.changed.length;
  return (
    <section className={`panel ${s.diff}`} aria-label={t.diff.title(from, to)}>
      <h2>{t.diff.title(from, to)}</h2>
      {empty ? <p className="muted">{t.diff.none}</p> : null}
      {diff.fields.length ? (
        <>
          <h3>{t.diff.fields}</h3>
          <ul>{diff.fields.map((f) => <li key={f}>{f}</li>)}</ul>
        </>
      ) : null}
      {diff.added.length ? (
        <>
          <h3>{t.diff.added}</h3>
          <ul>{diff.added.map((id) => <li key={id}>{id}</li>)}</ul>
        </>
      ) : null}
      {diff.removed.length ? (
        <>
          <h3>{t.diff.removed}</h3>
          <ul>{diff.removed.map((id) => <li key={id}>{id}</li>)}</ul>
        </>
      ) : null}
      {diff.changed.length ? (
        <>
          <h3>{t.diff.changed}</h3>
          <ul>{diff.changed.map((c) => <li key={c.id}>{c.id}: {c.fields.join(", ")}</li>)}</ul>
        </>
      ) : null}
    </section>
  );
}
