import type { Metadata } from "next";
import { Workbench } from "@/components/research/Workbench";
import type { PickerSession } from "@/components/research/SessionPicker";
import type { SessionSummary } from "@/lib/api";
import type { ConfigDocumentKind } from "@/lib/api/catalog";
import { authedApi, currentRole } from "@/lib/auth/server";
import { parseBaselines } from "@/lib/configDocs";
import { loadOrNotFound } from "@/lib/loaders";
import { can } from "@/lib/permissions";
import type { ProfileDoc } from "@/lib/research/registry";
import { researchText } from "@/messages/research";
import { loadPreviewRecordsAction, loadResearchFramesAction, previewCohortAction, promoteAction, runOnCohortAction, updateExperimentAction } from "../../actions";

const t = researchText;

export const metadata: Metadata = { title: t.workbench.title };

/** RUI-3: the most recent sessions offered for the preview. */
const PICKER_SESSIONS = 50;

/** `/[org]/research/experiments/[id]`: the workbench (RUI-1..5). */
export default async function ExperimentPage({ params }: { params: Promise<{ org: string; id: string }> }) {
  const { org, id } = await params;
  const role = await currentRole();
  const api = await authedApi();
  const published = (kind: ConfigDocumentKind) =>
    loadOrNotFound(() => api.listConfigDocuments(kind, { status: "published", limit: 1 })).then((p) => p.items[0]);
  const [experiment, profile, baselines, catalog, sessionPage, cohorts] = await Promise.all([
    loadOrNotFound(() => api.getExperiment(id)),
    published("scoring_profile"),
    published("normative_baselines"),
    published("stimulus_catalog"),
    api.listSessions(org, { limit: PICKER_SESSIONS }).then((p) => p.items).catch(() => [] as SessionSummary[]),
    api.listCohorts(org, { limit: 100 }).then((p) => p.items.filter((c) => !c.archived_at)).catch(() => []),
  ]);
  // RFL-6 lives on the session detail, not on the list row.
  const sessions: PickerSession[] = await Promise.all(
    sessionPage.map(async (s) => {
      const detail = await api.getSession(s.id).catch(() => null);
      return { id: s.id, token: s.participant_token, startedAt: s.started_at, framesAvailable: Boolean(detail?.research_frames?.available) };
    }),
  );
  const set = baselines ? parseBaselines(baselines.body) : null;
  return (
    <>
      <h1>{experiment.name}</h1>
      <Workbench
        orgId={org}
        experiment={experiment}
        publishedProfile={(profile?.body ?? null) as ProfileDoc | null}
        baselines={set}
        catalog={catalog?.body ?? null}
        sessions={sessions}
        cohorts={cohorts}
        canEdit={can(role, "experiment.write")}
        canRun={can(role, "analysis.run")}
        canPromote={can(role, "experiment.promote")}
        actions={{
          update: updateExperimentAction.bind(null, org, id),
          promote: promoteAction.bind(null, org, id),
          previewCohort: previewCohortAction.bind(null, org),
          runOnCohort: runOnCohortAction.bind(null, org),
          loadRecords: loadPreviewRecordsAction,
          loadFrames: loadResearchFramesAction,
        }}
      />
    </>
  );
}
