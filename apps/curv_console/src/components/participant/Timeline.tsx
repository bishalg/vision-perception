import Link from "next/link";
import { attentionFlagSchema } from "@/lib/api";
import { TIMELINE_KINDS, type TimelineEntry, type TimelineKind } from "@/lib/api/participant";
import { formatDateTime } from "@/lib/format";
import { participantText } from "@/messages/participant";
import { FlagBadge } from "../FlagBadge";

const t = participantText.timeline;

/** Parse repeated `?kind=` params into a validated, de-duplicated list (unknown values are dropped). */
export function parseKinds(value: string | string[] | undefined): TimelineKind[] {
  const raw = Array.isArray(value) ? value : value ? [value] : [];
  return TIMELINE_KINDS.filter((k) => raw.includes(k));
}

/** Kind-specific header text. Headers carry no free text (CW-3), so nothing here can leak note content. */
export function describeEntry(e: TimelineEntry): string {
  const s = e.summary;
  const str = (k: string) => (typeof s[k] === "string" ? (s[k] as string) : "");
  switch (e.kind) {
    case "session":
      return str("attention_flag") ? t.flag(str("attention_flag")) : t.kinds.session;
    case "clinical_review":
      return t.review(str("state") || "open", s.version_no);
    case "note":
      return t.note(str("note_kind"));
    case "letter":
      return t.letter(str("letter_kind"), s.status);
    case "care_event":
      return t.care(str("care_kind") || "event");
  }
}

function flagOf(e: TimelineEntry) {
  const parsed = e.kind === "session" ? attentionFlagSchema.safeParse(e.summary.attention_flag) : null;
  return parsed?.success ? parsed.data : null;
}

export function Timeline({
  entries,
  kinds,
  basePath,
  orgPath,
  nextCursor,
  hasCursor,
}: {
  entries: TimelineEntry[];
  kinds: TimelineKind[];
  /** `/org/participants/id`, the page the filter and paging links point back to. */
  basePath: string;
  /** `/org`, for session links. */
  orgPath: string;
  nextCursor: string | null | undefined;
  hasCursor: boolean;
}) {
  const shown = kinds.length ? entries.filter((e) => kinds.includes(e.kind)) : entries;
  const keep = new URLSearchParams();
  keep.set("tab", "timeline");
  for (const k of kinds) keep.append("kind", k);
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
              <input type="checkbox" name="kind" value={k} defaultChecked={kinds.includes(k)} /> {t.kinds[k]}
            </label>
          ))}
        </fieldset>
        <div className="filter-actions">
          <button type="submit">{t.apply}</button>
          {kinds.length ? <Link href={`${basePath}?tab=timeline`}>{t.clear}</Link> : null}
        </div>
      </form>
      {kinds.length ? <p className="muted">{t.filterNote}</p> : null}
      {shown.length === 0 ? (
        <p className="muted">{entries.length === 0 ? t.empty : t.emptyFiltered}</p>
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
              {shown.map((e) => (
                <tr key={`${e.kind}:${e.id}`} data-kind={e.kind}>
                  <td data-label={t.cols.when}>{formatDateTime(e.at)}</td>
                  <td data-label={t.cols.kind}>{t.kinds[e.kind]}</td>
                  <td data-label={t.cols.detail}>
                    {flagOf(e) ? <FlagBadge flag={flagOf(e)!} /> : describeEntry(e)}{" "}
                    {e.kind === "session" ? (
                      <Link href={`${orgPath}/sessions/${encodeURIComponent(e.id)}`}>{t.openSession}</Link>
                    ) : null}
                  </td>
                  <td data-label={t.cols.by}>{e.actor_display_name ?? "-"}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
      <nav className="pagination" aria-label="Timeline pages">
        {hasCursor ? <Link href={`${basePath}?tab=timeline${kinds.map((k) => `&kind=${k}`).join("")}`}>{t.newest}</Link> : null}
        {nextCursor ? (
          <Link rel="next" href={`${basePath}?${next.toString()}`}>
            {t.older}
          </Link>
        ) : null}
      </nav>
    </section>
  );
}
