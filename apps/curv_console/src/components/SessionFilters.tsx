"use client";

import Link from "next/link";
import { ATTENTION_FLAGS, QUALITY_VERDICTS, TRACKING_VERDICTS, type SessionListQuery } from "@/lib/api";
import { useText } from "@/lib/i18n/provider";
import { enumLabel } from "@/messages/domain";
import { pathsFor } from "@/lib/paths";

/** Plain GET form: works without client JS and keeps filter state in the URL. */
export function SessionFilters({ orgId, query }: { orgId: string; query: SessionListQuery }) {
  const t = useText();
  const f = t.app.sessions.filters;
  const reviewed = query.reviewed === undefined ? "" : String(query.reviewed);
  return (
    <form method="get" className="filters" aria-label={t.app.sessions.title}>
      <label>
        {f.flag}
        <select name="flag" defaultValue={query.flag ?? ""}>
          <option value="">{f.any}</option>
          {ATTENTION_FLAGS.map((x) => (
            <option key={x} value={x}>{enumLabel(t.locale, "attention_flag", x)}</option>
          ))}
        </select>
      </label>
      <label>
        {f.quality}
        <select name="quality" defaultValue={query.quality ?? ""}>
          <option value="">{f.any}</option>
          {QUALITY_VERDICTS.map((x) => (
            <option key={x} value={x}>{enumLabel(t.locale, "quality_verdict", x)}</option>
          ))}
        </select>
      </label>
      <label>
        {f.tracking}
        <select name="tracking" defaultValue={query.tracking ?? ""}>
          <option value="">{f.any}</option>
          {TRACKING_VERDICTS.map((x) => (
            <option key={x} value={x}>{enumLabel(t.locale, "tracking_verdict", x)}</option>
          ))}
        </select>
      </label>
      <label>
        {f.from}
        <input type="date" name="from" defaultValue={query.from ?? ""} />
      </label>
      <label>
        {f.to}
        <input type="date" name="to" defaultValue={query.to ?? ""} />
      </label>
      <label>
        {f.bundle}
        <input type="text" name="bundle_version" defaultValue={query.bundle_version ?? ""} maxLength={40} />
      </label>
      <label>
        {f.reviewed}
        <select name="reviewed" defaultValue={reviewed}>
          <option value="">{f.any}</option>
          <option value="true">{f.yes}</option>
          <option value="false">{f.no}</option>
        </select>
      </label>
      <div className="filter-actions">
        <button type="submit">{f.apply}</button>
        <Link href={pathsFor(orgId).clinic.sessions}>{f.reset}</Link>
      </div>
    </form>
  );
}
