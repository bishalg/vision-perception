"use client";

import Link from "next/link";
import type { AuditQuery } from "@/lib/api";
import { useText } from "@/lib/i18n/provider";

/** Plain GET form: works without client JS and keeps the filters in the URL (the backend validates them). */
export function AuditFilters({ orgId, query }: { orgId: string; query: AuditQuery }) {
  const f = useText().app.audit.filters;
  return (
    <form method="get" className="filters" aria-label={f.label}>
      <label>
        {f.actor}
        <input type="text" name="actor_id" defaultValue={query.actor_id ?? ""} maxLength={200} autoComplete="off" />
      </label>
      <label>
        {f.action}
        <input
          type="text"
          name="action"
          defaultValue={query.action ?? ""}
          maxLength={200}
          autoComplete="off"
          placeholder={f.actionHint}
        />
      </label>
      <label>
        {f.targetType}
        <input type="text" name="target_type" defaultValue={query.target_type ?? ""} maxLength={200} autoComplete="off" />
      </label>
      <label>
        {f.targetId}
        <input type="text" name="target_id" defaultValue={query.target_id ?? ""} maxLength={200} autoComplete="off" />
      </label>
      <label>
        {f.from}
        <input type="date" name="from" defaultValue={query.from?.slice(0, 10) ?? ""} />
      </label>
      <label>
        {f.to}
        <input type="date" name="to" defaultValue={query.to?.slice(0, 10) ?? ""} />
      </label>
      <div className="filter-actions">
        <button type="submit">{f.apply}</button>
        <Link href={`/${encodeURIComponent(orgId)}/audit`}>{f.reset}</Link>
      </div>
    </form>
  );
}
