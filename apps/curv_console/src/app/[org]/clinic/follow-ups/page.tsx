import type { Metadata } from "next";
import Link from "next/link";
import { FollowUpsTable } from "@/components/participant/FollowUpsTable";
import type { FollowUpQuery } from "@/lib/api";
import { authedApi, currentRole } from "@/lib/auth/server";
import { parseCursor } from "@/lib/listQuery";
import { loadOrNotFound, requestTime } from "@/lib/loaders";
import { can } from "@/lib/permissions";
import { getText } from "@/lib/i18n/server";
import { markFollowUpDoneAction } from "./actions";
import { orgBase, orgPaths } from "@/lib/paths";

export async function generateMetadata(): Promise<Metadata> {
  return { title: (await getText()).participant.followUps.title };
}

const SHOWS = ["all", "overdue", "week"] as const;
type Show = (typeof SHOWS)[number];
const DAY = 86_400_000;

/**
 * Open follow-ups of the organisation (REC-4), earliest due first. The filters are enum values in the query string
 * (no free text), so they can live in the URL.
 */
export default async function FollowUpsPage({
  params,
  searchParams,
}: {
  params: Promise<{ org: string }>;
  searchParams: Promise<Record<string, string | string[] | undefined>>;
}) {
  const { org } = await params;
  const text = await getText();
  const t = text.participant.followUps;
  const sp = await searchParams;
  const one = (k: string) => (Array.isArray(sp[k]) ? sp[k]![0] : (sp[k] as string | undefined));
  const show: Show = (SHOWS as readonly string[]).includes(one("show") ?? "") ? (one("show") as Show) : "all";
  const mine = one("mine") === "1";
  const cursor = parseCursor(sp);
  const now = requestTime();
  const query: FollowUpQuery = {
    limit: 50,
    ...(cursor ? { cursor } : {}),
    ...(show === "overdue" ? { due_before: new Date(now).toISOString() } : {}),
    ...(show === "week" ? { due_before: new Date(now + 7 * DAY).toISOString() } : {}),
    ...(mine ? { assigned: "me" as const } : {}),
  };
  const api = await authedApi();
  const role = await currentRole();
  const page = await loadOrNotFound(() => api.listFollowUps(org, query));
  const orgPath = orgBase(org);
  const keep = new URLSearchParams();
  if (show !== "all") keep.set("show", show);
  if (mine) keep.set("mine", "1");
  const next = new URLSearchParams(keep);
  if (page.next_cursor) next.set("cursor", page.next_cursor);
  const base = orgPaths(orgPath).clinic.followUps;
  return (
    <>
      <h1>{t.title}</h1>
      <p className="muted">{t.intro}</p>
      <form method="get" className="filter-actions" aria-label={t.filterLabel}>
        <label>
          {t.show}{" "}
          <select name="show" defaultValue={show}>
            <option value="all">{t.all}</option>
            <option value="overdue">{t.overdue}</option>
            <option value="week">{t.week}</option>
          </select>
        </label>
        <label>
          <input type="checkbox" name="mine" value="1" defaultChecked={mine} /> {t.mine}
        </label>
        <button type="submit">{t.apply}</button>
      </form>
      <FollowUpsTable
        rows={page.items}
        now={now}
        orgPath={orgPath}
        canWrite={can(role, "encounter.write")}
        done={markFollowUpDoneAction.bind(null, org)}
        filtered={show !== "all" || mine}
      />
      <nav className="pagination" aria-label={text.participant.pagination}>
        {cursor ? <Link href={`${base}${keep.size ? `?${keep}` : ""}`}>{t.first}</Link> : null}
        {page.next_cursor ? (
          <Link rel="next" href={`${base}?${next}`}>{t.older}</Link>
        ) : null}
      </nav>
    </>
  );
}
