"use client";

import Link from "next/link";
import type { Integration } from "@/lib/api/exports";
import { formatDateTime } from "@/lib/format";
import { useText } from "@/lib/i18n/provider";

/** Last test outcome: pass/fail with the error class (never remote text), or "Never run". */
export function TestBadge({ i }: { i: Pick<Integration, "last_test_at" | "last_test_ok" | "last_test_error"> }) {
  const t = useText().exports.integrations;
  if (!i.last_test_at || i.last_test_ok == null) return <span className="muted">{t.testNever}</span>;
  return (
    <>
      <span className={`badge ${i.last_test_ok ? "badge-green" : "badge-red"}`}>
        {i.last_test_ok ? t.testOk : t.testFailed}
      </span>{" "}
      <span className="muted">
        {formatDateTime(i.last_test_at)}
        {i.last_test_error ? ` (${i.last_test_error})` : ""}
      </span>
    </>
  );
}

export function IntegrationsTable({ orgId, items }: { orgId: string; items: Integration[] }) {
  const t = useText().exports.integrations;
  const c = t.cols;
  if (items.length === 0) return <p className="muted">{t.empty}</p>;
  return (
    <div className="table-wrap">
      <table className="data responsive">
        <thead>
          <tr>
            <th scope="col">{c.name}</th>
            <th scope="col">{c.kind}</th>
            <th scope="col">{c.environment}</th>
            <th scope="col">{c.status}</th>
            <th scope="col">{c.credential}</th>
            <th scope="col">{c.lastTest}</th>
            <th scope="col">{c.actions}</th>
          </tr>
        </thead>
        <tbody>
          {items.map((i) => (
            <tr key={i.id}>
              <td data-label={c.name}>{i.name}</td>
              <td data-label={c.kind}>{t.kinds[i.kind]}</td>
              <td data-label={c.environment}>{t.environments[i.environment]}</td>
              <td data-label={c.status}>
                <span className={`badge ${i.status === "active" ? "badge-green" : "badge-unreliable"}`}>{t.statuses[i.status]}</span>
              </td>
              <td data-label={c.credential}>{i.has_secret ? t.credentialSet : t.credentialMissing}</td>
              <td data-label={c.lastTest}><TestBadge i={i} /></td>
              <td data-label={c.actions}>
                <Link href={`/${encodeURIComponent(orgId)}/settings/integrations/${encodeURIComponent(i.id)}`}>{t.manage}</Link>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
