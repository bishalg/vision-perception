import type { Metadata } from "next";
import Link from "next/link";
import { ReviewQueue, type ReviewRow } from "@/components/ReviewQueue";
import { REVIEW_REASONS, REVIEW_STATUSES } from "@/lib/api";
import { authedApi, currentRole } from "@/lib/auth/server";
import { parseReviewQuery, toSearch } from "@/lib/listQuery";
import { loadOrNotFound, requestTime } from "@/lib/loaders";
import { can } from "@/lib/permissions";
import { en } from "@/messages/en";
import { triageAction } from "./actions";

export const metadata: Metadata = { title: en.review.title };

const PAGE_SIZE = 25;
const t = en.review;

export default async function ReviewPage({
  params,
  searchParams,
}: {
  params: Promise<{ org: string }>;
  searchParams: Promise<Record<string, string | string[] | undefined>>;
}) {
  const { org } = await params;
  const query = parseReviewQuery(await searchParams);
  const api = await authedApi();
  const role = await currentRole();
  const page = await loadOrNotFound(() =>
    api.listReview(org, { status: query.status, reason: query.reason, cursor: query.cursor, limit: PAGE_SIZE }),
  );
  // ReviewItem carries only session_id; the flag/quality/participant columns come from the session itself
  // (one read per item; see README "Backend gaps"). A failed read leaves the row without them.
  const sessions = await Promise.allSettled(page.items.map((i) => api.getSession(i.session_id)));
  const rows: ReviewRow[] = page.items.map((item, k) => {
    const s = sessions[k];
    return {
      item,
      session:
        s?.status === "fulfilled"
          ? {
              participant_token: s.value.participant_token,
              attention_flag: s.value.attention_flag,
              quality_verdict: s.value.quality_verdict,
              started_at: s.value.started_at,
            }
          : null,
    };
  });
  const statusParam = query.all ? "all" : query.status;
  const base = `/${encodeURIComponent(org)}/review`;
  const triage = triageAction.bind(null, org);
  return (
    <>
      <h1>{t.title}</h1>
      <form method="get" className="filters" aria-label={t.filters.label}>
        <label>
          {t.filters.status}
          <select name="status" defaultValue={statusParam}>
            {REVIEW_STATUSES.map((s) => (
              <option key={s} value={s}>{t.statuses[s]}</option>
            ))}
            <option value="all">{t.filters.any}</option>
          </select>
        </label>
        <label>
          {t.filters.reason}
          <select name="reason" defaultValue={query.reason ?? ""}>
            <option value="">{t.filters.any}</option>
            {REVIEW_REASONS.map((r) => (
              <option key={r} value={r}>{t.reasons[r]}</option>
            ))}
          </select>
        </label>
        <div className="filter-actions">
          <button type="submit">{t.filters.apply}</button>
        </div>
      </form>
      <ReviewQueue
        key={`${statusParam}|${query.reason ?? ""}|${query.cursor ?? ""}`}
        orgId={org}
        rows={rows}
        canWrite={can(role, "review.write")}
        now={requestTime()}
        filterStatus={query.status}
        triage={triage}
      />
      <nav className="pagination" aria-label="Pagination">
        <span className="muted">{t.showing(page.items.length, page.total)}</span>
        {query.cursor ? (
          <Link href={`${base}${toSearch({ status: statusParam, reason: query.reason })}`}>{en.sessions.first}</Link>
        ) : null}
        {page.next_cursor ? (
          <Link rel="next" href={`${base}${toSearch({ status: statusParam, reason: query.reason, cursor: page.next_cursor })}`}>
            {en.sessions.next}
          </Link>
        ) : null}
      </nav>
    </>
  );
}
