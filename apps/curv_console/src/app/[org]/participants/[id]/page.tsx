import type { Metadata } from "next";
import Link from "next/link";
import { notFound } from "next/navigation";
import "@/components/participant/participant.css";
import { AttachmentsPanel } from "@/components/participant/AttachmentsPanel";
import { Disclosures } from "@/components/participant/Disclosures";
import { EncountersPanel } from "@/components/participant/EncountersPanel";
import { MrnPanel } from "@/components/participant/MrnPanel";
import { NotesPanel } from "@/components/participant/NotesPanel";
import { ParticipantHeader } from "@/components/participant/ParticipantHeader";
import { ParticipantTabs, parseWorkspaceTab } from "@/components/participant/ParticipantTabs";
import { PrintSummary, type PrintRow } from "@/components/participant/PrintSummary";
import { Timeline, parseDay, parseKinds } from "@/components/participant/Timeline";
import { ApiError, type ClinicalReview, type CurvApi } from "@/lib/api";
import { authedApi, currentRole } from "@/lib/auth/server";
import { formatDateTime } from "@/lib/format";
import { parseCursor } from "@/lib/listQuery";
import { loadOrNotFound, requestTime } from "@/lib/loaders";
import { can } from "@/lib/permissions";
import { participantText, type WorkspaceTab } from "@/messages/participant";
import {
  attachmentUploadedAction,
  createAttachmentAction,
  createEncounterAction,
  createNoteAction,
  deleteAttachmentAction,
  encounterSessionAction,
  encounterStateAction,
  updateEncounterAction,
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

  const canFiles = canContent || can(role, "attachment.write");
  const tabs: WorkspaceTab[] = [
    "timeline",
    "visits",
    ...(canFiles ? (["attachments"] as const) : []),
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

  async function sessionChoices() {
    const page = await loadOrNotFound(() => api.listSessions(org, { participant_id: id, limit: 100 }));
    return page.items.map((s) => ({ id: s.id, label: formatDateTime(s.started_at) }));
  }

  async function panel() {
    switch (tab) {
      case "timeline": {
        const kinds = parseKinds(sp.kind);
        const from = parseDay(sp.from);
        const to = parseDay(sp.to);
        const page = await loadOrNotFound(() =>
          api.getParticipantTimeline(id, { cursor, limit: 25, ...(kinds.length ? { kind: kinds } : {}), from, to }),
        );
        return (
          <Timeline
            entries={page.items}
            kinds={kinds}
            from={from}
            to={to}
            basePath={basePath}
            orgPath={orgPath}
            nextCursor={page.next_cursor}
            hasCursor={Boolean(cursor)}
          />
        );
      }
      case "visits": {
        const [page, sessions] = await Promise.all([
          loadOrNotFound(() => api.listEncounters(id, { cursor, limit: 25 })),
          sessionChoices(),
        ]);
        const active = !participant.withdrawn_at;
        return (
          <>
            <EncountersPanel
              encounters={page.items}
              sessions={sessions}
              canWrite={can(role, "encounter.write") && active}
              canSummary={can(role, "note.write") && active}
              canContent={canContent}
              // REC-4: owner or clinician reopen a closed visit (the backend answers 409 otherwise).
              canReopen={role === "owner" || role === "clinician"}
              actions={{
                create: createEncounterAction.bind(null, org, id),
                update: updateEncounterAction.bind(null, org, id),
                state: encounterStateAction.bind(null, org, id),
                session: encounterSessionAction.bind(null, org, id),
              }}
              now={requestTime()}
              orgPath={orgPath}
            />
            <nav className="pagination" aria-label="Visit pages">
              {cursor ? <Link href={`${basePath}?tab=visits`}>{t.visits.newest}</Link> : null}
              {page.next_cursor ? (
                <Link rel="next" href={`${basePath}?tab=visits&cursor=${encodeURIComponent(page.next_cursor)}`}>
                  {t.visits.older}
                </Link>
              ) : null}
            </nav>
          </>
        );
      }
      case "attachments": {
        const [files, visits] = await Promise.all([
          loadOrNotFound(() => api.listAttachments(id, { cursor, limit: 50 })),
          loadOrNotFound(() => api.listEncounters(id, { limit: 50 })),
        ]);
        return (
          <>
            <AttachmentsPanel
              rows={files.items}
              canRead={canContent}
              canWrite={can(role, "attachment.write") && !participant.withdrawn_at}
              actions={{
                create: createAttachmentAction.bind(null, org, id),
                remove: deleteAttachmentAction.bind(null, org, id),
                uploaded: attachmentUploadedAction.bind(null, org, id),
              }}
              visits={visits.items.map((v) => ({
                id: v.id,
                label: `${t.visits.kinds[v.kind]}, ${formatDateTime(v.occurred_at)}`,
              }))}
            />
            <nav className="pagination" aria-label="File pages">
              {cursor ? <Link href={`${basePath}?tab=attachments`}>{t.attachments.newest}</Link> : null}
              {files.next_cursor ? (
                <Link rel="next" href={`${basePath}?tab=attachments&cursor=${encodeURIComponent(files.next_cursor)}`}>
                  {t.attachments.older}
                </Link>
              ) : null}
            </nav>
          </>
        );
      }
      case "notes": {
        const [page, sessionPage] = await Promise.all([
          loadOrNotFound(() => api.listParticipantNotes(id, { cursor, limit: 25 })),
          loadOrNotFound(() => api.listSessions(org, { participant_id: id, limit: 100 })),
        ]);
        const sessions = sessionPage.items.map((s) => ({ id: s.id, label: formatDateTime(s.started_at) }));
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
        // One extra row tells whether the list was cut at PRINT_SESSIONS.
        const sessionPage = await loadOrNotFound(() =>
          api.listSessions(org, { participant_id: id, limit: PRINT_SESSIONS + 1 }),
        );
        const used = sessionPage.items.slice(0, PRINT_SESSIONS);
        const rows: PrintRow[] = await Promise.all(
          used.map(async (s) => ({
            sessionId: s.id,
            at: s.started_at,
            flag: s.attention_flag,
            review: canContent ? await reviewOf(api, s.id) : undefined,
          })),
        );
        const care = canContent ? await loadOrNotFound(() => api.getParticipantCareSummary(id)) : undefined;
        const notes = canContent
          ? (await loadOrNotFound(() => api.listParticipantNotes(id, { limit: 100 }))).items.filter((n) => !n.corrected_by)
          : undefined;
        return (
          <PrintSummary
            participant={participant}
            rows={rows}
            notes={notes}
            care={care}
            truncated={sessionPage.items.length > used.length ? used.length : undefined}
            generatedAt={new Date(requestTime()).toISOString()}
          />
        );
      }
    }
  }
}
