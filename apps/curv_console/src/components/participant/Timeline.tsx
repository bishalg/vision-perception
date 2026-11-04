"use client";

import Link from "next/link";
import { TIMELINE_KINDS, type TimelineEntry, type TimelineKind } from "@/lib/api/participant";
import { formatDateTime } from "@/lib/format";
import { useText } from "@/lib/i18n/provider";
import { FlagBadge } from "../FlagBadge";
import { describeEntry, entryTab, flagOf } from "./timelineQuery";

// The query parsers and `describeEntry` live in `timelineQuery.ts` (a server page calls them); re-exported here.
export { describeEntry, parseDay, parseKinds } from "./timelineQuery";

/**
 * The timeline page. `entries` are already filtered by the server (`kind`, `from`, `to` of
 * `GET /participants/{id}/timeline`), so the cursor continues inside the filter.
 */
export function Timeline({
  entries,
  kinds,
  from,
  to,
  basePath,
  orgPath,
  nextCursor,
  hasCursor,
}: {
  entries: TimelineEntry[];
  kinds: TimelineKind[];
  /** Inclusive calendar days (UTC) the server filtered by. */
  from?: string;
  to?: string;
  /** `/org/participants/id`, the page the filter and paging links point back to. */
  basePath: string;
  /** `/org`, for session links. */
  orgPath: string;
  nextCursor: string | null | undefined;
  hasCursor: boolean;
}) {
  const text = useText();
  const t = text.participant.timeline;
  const kindLabel = (k: TimelineKind) => text.domain.enumLabel(text.locale, "timeline_entry_kind", k);
  const filtered = kinds.length > 0 || Boolean(from) || Boolean(to);
  const keep = new URLSearchParams();
  keep.set("tab", "timeline");
  for (const k of kinds) keep.append("kind", k);
  if (from) keep.set("from", from);
  if (to) keep.set("to", to);
  const next = new URLSearchParams(keep);
  if (nextCursor) next.set("cursor", nextCursor);
  return (
    <section aria-labelledby="timeline-h">
      <h2 id="timeline-h">{t.title}</h2>
      <p className="muted">{t.intro}</p>
      <form method="get" className="timeline-filter" aria-label={t.filterLegend}>
        <input type="hidden" name="tab" value="timeline" />
        <fieldset>
          <legend>{t.filterLegend}</legend>
          {TIMELINE_KINDS.map((k) => (
            <label key={k} className="check">
              <input type="checkbox" name="kind" value={k} defaultChecked={kinds.includes(k)} /> {kindLabel(k)}
            </label>
          ))}
        </fieldset>
        <label>
          {t.from} <input type="date" name="from" defaultValue={from ?? ""} />
        </label>
        <label>
          {t.to} <input type="date" name="to" defaultValue={to ?? ""} />
        </label>
        <div className="filter-actions">
          <button type="submit">{t.apply}</button>
          {filtered ? <Link href={`${basePath}?tab=timeline`}>{t.clear}</Link> : null}
        </div>
      </form>
      {entries.length === 0 ? (
        <p className="muted">{filtered ? t.emptyFiltered : t.empty}</p>
      ) : (
        <div className="table-wrap">
          <table className="data responsive">
            <thead>
              <tr>
                <th scope="col">{t.cols.when}</th>
                <th scope="col">{t.cols.kind}</th>
                <th scope="col">{t.cols.detail}</th>
                <th scope="col">{t.cols.by}</th>
              </tr>
            </thead>
            <tbody>
              {entries.map((e) => (
                <tr key={`${e.kind}:${e.id}`} data-kind={e.kind}>
                  <td data-label={t.cols.when}>{formatDateTime(e.at)}</td>
                  <td data-label={t.cols.kind}>{kindLabel(e.kind)}</td>
                  <td data-label={t.cols.detail}>
                    {flagOf(e) ? <FlagBadge flag={flagOf(e)!} /> : describeEntry(e, text)}{" "}
                    {e.kind === "session" ? (
                      <Link href={`${orgPath}/sessions/${encodeURIComponent(e.id)}`}>{t.openSession}</Link>
                    ) : entryTab(e.kind) ? (
                      <Link href={`${basePath}?tab=${entryTab(e.kind)}`}>
                        {entryTab(e.kind) === "visits" ? t.openVisits : t.openFiles}
                      </Link>
                    ) : null}
                  </td>
                  <td data-label={t.cols.by}>{e.actor_display_name ?? "-"}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
      <nav className="pagination" aria-label={t.pages}>
        {hasCursor ? <Link href={`${basePath}?${keep.toString()}`}>{t.newest}</Link> : null}
        {nextCursor ? (
          <Link rel="next" href={`${basePath}?${next.toString()}`}>
            {t.older}
          </Link>
        ) : null}
      </nav>
    </section>
  );
}
