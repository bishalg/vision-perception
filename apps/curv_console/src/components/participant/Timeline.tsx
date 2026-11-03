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
    // REC-5 headers as B.19 writes them (`read/timeline.rs`): `{encounter_kind, status}` (the clinician is the "By"
    // column) and `{attachment_kind, file_name, byte_size}`. Never the visit summary or reason.
    case "encounter":
      return t.encounter(str("encounter_kind"), s.status);
    case "attachment":
      return t.attachment(str("attachment_kind"), str("file_name"), typeof s.byte_size === "number" ? s.byte_size : null);
  }
}

/** The workspace tab an entry opens, for kinds that have one. */
function entryTab(kind: TimelineKind): "visits" | "attachments" | null {
  return kind === "encounter" ? "visits" : kind === "attachment" ? "attachments" : null;
}

function flagOf(e: TimelineEntry) {
  const parsed = e.kind === "session" ? attentionFlagSchema.safeParse(e.summary.attention_flag) : null;
  return parsed?.success ? parsed.data : null;
}

const DATE_RE = /^\d{4}-\d{2}-\d{2}$/;

/** A `?from=` / `?to=` calendar day (`YYYY-MM-DD`); anything else is dropped. */
export function parseDay(value: string | string[] | undefined): string | undefined {
  const v = Array.isArray(value) ? value[0] : value;
  return v && DATE_RE.test(v) ? v : undefined;
}

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
              <input type="checkbox" name="kind" value={k} defaultChecked={kinds.includes(k)} /> {t.kinds[k]}
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
                  <td data-label={t.cols.kind}>{t.kinds[e.kind]}</td>
                  <td data-label={t.cols.detail}>
                    {flagOf(e) ? <FlagBadge flag={flagOf(e)!} /> : describeEntry(e)}{" "}
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
      <nav className="pagination" aria-label="Timeline pages">
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
