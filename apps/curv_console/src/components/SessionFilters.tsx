import Link from "next/link";
import { en } from "@/messages/en";
import { ATTENTION_FLAGS, QUALITY_VERDICTS, type SessionListQuery } from "@/lib/api";

const f = en.sessions.filters;

/** Plain GET form: works without client JS and keeps filter state in the URL. */
export function SessionFilters({ orgId, query }: { orgId: string; query: SessionListQuery }) {
  const reviewed = query.reviewed === undefined ? "" : String(query.reviewed);
  return (
    <form method="get" className="filters" aria-label={en.sessions.title}>
      <label>
        {f.flag}
        <select name="flag" defaultValue={query.flag ?? ""}>
          <option value="">{f.any}</option>
          {ATTENTION_FLAGS.map((x) => (
            <option key={x} value={x}>{en.flags[x]}</option>
          ))}
        </select>
      </label>
      <label>
        {f.quality}
        <select name="quality" defaultValue={query.quality ?? ""}>
          <option value="">{f.any}</option>
          {QUALITY_VERDICTS.map((x) => (
            <option key={x} value={x}>{x}</option>
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
        <Link href={`/${encodeURIComponent(orgId)}/sessions`}>{f.reset}</Link>
      </div>
    </form>
  );
}
