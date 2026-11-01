import type { Metadata } from "next";
import Link from "next/link";
import { notFound } from "next/navigation";
import "@/components/participant/participant.css";
import { Disclosures } from "@/components/participant/Disclosures";
import { MrnPanel } from "@/components/participant/MrnPanel";
import { NotesPanel } from "@/components/participant/NotesPanel";
import { ParticipantHeader } from "@/components/participant/ParticipantHeader";
import { ParticipantTabs, parseWorkspaceTab } from "@/components/participant/ParticipantTabs";
import { PrintSummary, type PrintRow } from "@/components/participant/PrintSummary";
import { Timeline, parseKinds } from "@/components/participant/Timeline";
import { ApiError, attentionFlagSchema, type ClinicalReview, type CurvApi, type TimelineEntry } from "@/lib/api";
import { authedApi, currentRole } from "@/lib/auth/server";
import { formatDateTime } from "@/lib/format";
import { parseCursor } from "@/lib/listQuery";
import { loadOrNotFound, requestTime } from "@/lib/loaders";
import { can } from "@/lib/permissions";
import { participantText, type WorkspaceTab } from "@/messages/participant";
import {
  createNoteAction,
  revealMrnAction,
  saveMrnAction,
  searchMrnAction,
  unlinkMrnAction,
} from "./actions";

const t = participantText;

// No token, MRN or note text in the title (UI-4).
export const metadata: Metadata = { title: t.title };

const PRINT_SESSIONS = 20;

async function reviewOf(api: CurvApi, sessionId: string): Promise<ClinicalReview | null | undefined> {
  try {
    return await api.getSessionClinicalReview(sessionId);
  } catch (err) {
    if (err instanceof ApiError && (err.status === 403 || err.status === 404)) return undefined;
    throw err;
  }
}

export default async function ParticipantWorkspacePage({
  params,
  searchParams,
}: {
  params: Promise<{ org: string; id: string }>;
  searchParams: Promise<Record<string, string | string[] | undefined>>;
}) {
  const { org, id } = await params;
  const sp = await searchParams;
  const api = await authedApi();
  const role = await currentRole();
  const canContent = can(role, "clinical.content");
  const canMrn = can(role, "patientref.read");

  const participant = await loadOrNotFound(() => api.getParticipant(id));
  if (participant.org_id !== org) notFound();

  const tabs: WorkspaceTab[] = [
    "timeline",
    ...(canContent ? (["notes"] as const) : []),
    ...(canMrn ? (["mrn"] as const) : []),
    ...(canContent ? (["letters", "disclosures"] as const) : []),
    "print",
  ];
  const tab = parseWorkspaceTab(sp.tab, tabs);
  const cursor = parseCursor(sp);
  const orgPath = `/${encodeURIComponent(org)}`;
  const basePath = `${orgPath}/participants/${encodeURIComponent(id)}`;

  return (
    <>
      <p className="no-print">
        <Link href={`${orgPath}/participants`}>{t.back}</Link>
      </p>
      <ParticipantHeader participant={participant} />
      <ParticipantTabs basePath={basePath} active={tab} tabs={tabs} />
      {await panel()}
    </>
  );

  async function panel() {
    switch (tab) {
      case "timeline": {
        const page = await loadOrNotFound(() => api.getParticipantTimeline(id, { cursor, limit: 25 }));
        return (
          <Timeline
            entries={page.items}
            kinds={parseKinds(sp.kind)}
            basePath={basePath}
            orgPath={orgPath}
            nextCursor={page.next_cursor}
            hasCursor={Boolean(cursor)}
          />
        );
      }
      case "notes": {
        const [page, timeline] = await Promise.all([
          loadOrNotFound(() => api.listParticipantNotes(id, { cursor, limit: 25 })),
          loadOrNotFound(() => api.getParticipantTimeline(id, { limit: 100 })),
        ]);
        const sessions = timeline.items
          .filter((e) => e.kind === "session")
          .map((e) => ({ id: e.id, label: formatDateTime(e.at) }));
        return (
          <>
            <NotesPanel
              notes={page.items}
              total={page.total}
              sessions={sessions}
              canWrite={can(role, "note.write") && !participant.withdrawn_at}
              create={createNoteAction.bind(null, org, id)}
            />
            <nav className="pagination" aria-label="Note pages">
              {cursor ? <Link href={`${basePath}?tab=notes`}>{t.notes.newest}</Link> : null}
              {page.next_cursor ? (
                <Link rel="next" href={`${basePath}?tab=notes&cursor=${encodeURIComponent(page.next_cursor)}`}>
                  {t.notes.older}
                </Link>
              ) : null}
            </nav>
          </>
        );
      }
      case "mrn": {
        const orgInfo = await loadOrNotFound(() => api.getOrg(org));
        const identity = (orgInfo.settings as { identity?: { patient_ref_mode?: string } } | null | undefined)?.identity;
        const mode = identity?.patient_ref_mode === "mrn" ? "mrn" : identity?.patient_ref_mode === "none" ? "none" : "unknown";
        return (
          <MrnPanel
            mode={mode}
            hasRef={participant.has_patient_ref}
            canRead={canMrn}
            canWrite={can(role, "patientref.write") && !participant.withdrawn_at}
            reveal={revealMrnAction.bind(null, id)}
            save={saveMrnAction.bind(null, org, id)}
            unlink={unlinkMrnAction.bind(null, org, id)}
            search={searchMrnAction.bind(null, org)}
          />
        );
      }
      case "letters":
        return (
          <section aria-labelledby="letters-h">
            <h2 id="letters-h">{t.letters.title}</h2>
            <p className="muted" data-testid="letters-soon">{t.letters.soon}</p>
          </section>
        );
      case "disclosures": {
        const page = await loadOrNotFound(() => api.listParticipantDisclosures(id, { cursor, limit: 25 }));
        return (
          <Disclosures items={page.items} basePath={basePath} nextCursor={page.next_cursor} hasCursor={Boolean(cursor)} />
        );
      }
      case "print": {
        const timeline = await loadOrNotFound(() => api.getParticipantTimeline(id, { limit: 100 }));
        const sessionEntries: TimelineEntry[] = timeline.items.filter((e) => e.kind === "session");
        const used = sessionEntries.slice(0, PRINT_SESSIONS);
        const rows: PrintRow[] = await Promise.all(
          used.map(async (e) => ({
            sessionId: e.id,
            at: e.at,
            flag: (() => {
              const f = attentionFlagSchema.safeParse(e.summary.attention_flag);
              return f.success ? f.data : null;
            })(),
            review: canContent ? await reviewOf(api, e.id) : undefined,
          })),
        );
        const notes = canContent
          ? (await loadOrNotFound(() => api.listParticipantNotes(id, { limit: 100 }))).items.filter((n) => !n.corrected_by)
          : undefined;
        return (
          <PrintSummary
            participant={participant}
            rows={rows}
            notes={notes}
            truncated={sessionEntries.length > used.length ? used.length : undefined}
            generatedAt={new Date(requestTime()).toISOString()}
          />
        );
      }
    }
  }
}
