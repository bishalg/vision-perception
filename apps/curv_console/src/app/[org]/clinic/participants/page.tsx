import type { Metadata } from "next";
import Link from "next/link";
import "@/components/participant/participant.css";
import { CreateParticipantDialog } from "@/components/participant/CreateParticipantDialog";
import { ParticipantSearch } from "@/components/participant/ParticipantSearch";
import { ParticipantsTable } from "@/components/ParticipantsTable";
import { authedApi, currentRole } from "@/lib/auth/server";
import { parseCursor, toSearch } from "@/lib/listQuery";
import { loadOrNotFound } from "@/lib/loaders";
import { can } from "@/lib/permissions";
import { getText } from "@/lib/i18n/server";
import { requestErasureAction } from "../privacy-actions";
import { createParticipantAction, searchParticipantsAction } from "./actions";

// No token or search value in the title (UI-4).
export async function generateMetadata(): Promise<Metadata> {
  return { title: (await getText()).admin.participants.title };
}

export default async function ParticipantsPage({
  params,
  searchParams,
}: {
  params: Promise<{ org: string }>;
  searchParams: Promise<Record<string, string | string[] | undefined>>;
}) {
  const { org } = await params;
  const text = await getText();
  const t = text.admin.participants;
  const cursor = parseCursor(await searchParams);
  const api = await authedApi();
  const role = await currentRole();
  const page = await loadOrNotFound(() => api.listParticipants(org, { cursor, limit: 50 }));
  const orgPath = `/${encodeURIComponent(org)}`;
  const base = `${orgPath}/participants`;
  return (
    <>
      <div className="filter-actions">
        <h1>{t.title}</h1>
        {can(role, "participant.create") ? (
          <CreateParticipantDialog create={createParticipantAction.bind(null, org)} orgPath={orgPath} />
        ) : null}
      </div>
      <p className="muted">{t.intro}</p>
      <ParticipantSearch search={searchParticipantsAction.bind(null, org)} canMrn={can(role, "patientref.read")} orgPath={orgPath} />
      <h2>{t.allTitle}</h2>
      <ParticipantsTable
        participants={page.items}
        orgId={org}
        erase={can(role, "participant.erase") ? requestErasureAction.bind(null, org) : undefined}
      />
      <p className="muted">{t.showing(page.items.length, page.total)}</p>
      <nav className="pagination" aria-label={text.participant.pagination}>
        {cursor ? <Link href={base}>{text.admin.common.first}</Link> : null}
        {page.next_cursor ? (
          <Link rel="next" href={`${base}${toSearch({ cursor: page.next_cursor })}`}>{text.admin.common.next}</Link>
        ) : null}
      </nav>
    </>
  );
}
