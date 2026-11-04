"use client";

import Link from "next/link";
import { ConfigStatusBadge } from "@/components/catalog/CatalogTables";
import { authorText } from "@/components/catalog/authorText";
import type { ConfigDocument } from "@/lib/api/catalog";
import { formatDateTime } from "@/lib/format";
import { useText } from "@/lib/i18n/provider";

export function DocVersionsTable({ base, items }: { base: string; items: ConfigDocument[] }) {
  const tx = useText();
  const t = tx.config;
  const ct = tx.catalog;
  if (!items.length) return <p className="muted">{ct.empty}</p>;
  return (
    <div className="table-wrap">
      <table className="data">
        <thead>
          <tr>
            <th>{ct.cols.version}</th>
            <th>{ct.cols.status}</th>
            <th>{ct.cols.created}</th>
            <th>{ct.cols.author}</th>
            <th>{ct.cols.published}</th>
            <th>{t.facts.notes}</th>
          </tr>
        </thead>
        <tbody>
          {items.map((d) => (
            <tr key={d.id}>
              <td><Link href={`${base}/${encodeURIComponent(d.version)}`}>{d.version}</Link></td>
              <td><ConfigStatusBadge status={d.status} /></td>
              <td>{formatDateTime(d.created_at)}</td>
              <td title={d.created_by ?? undefined}>{authorText(d.created_by, ct.system)}</td>
              <td>{d.published_at ? formatDateTime(d.published_at) : "—"}</td>
              <td style={{ whiteSpace: "normal" }}>{d.notes ?? ""}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
