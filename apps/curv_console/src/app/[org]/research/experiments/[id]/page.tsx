import type { Metadata } from "next";
import { Workbench } from "@/components/research/Workbench";
import type { SessionListItem } from "@/lib/api";
import type { ConfigDocumentKind } from "@/lib/api/catalog";
import { authedApi, currentRole } from "@/lib/auth/server";
import { parseBaselines } from "@/lib/configDocs";
import { getText } from "@/lib/i18n/server";
import { loadOrNotFound } from "@/lib/loaders";
import { can } from "@/lib/permissions";
import { pickerSessions } from "@/lib/research/preview";
import type { ProfileDoc } from "@/lib/research/registry";
import { loadPreviewRecordsAction, loadResearchFramesAction, previewCohortAction, promoteAction, runOnCohortAction, updateExperimentAction } from "../../actions";


export async function generateMetadata(): Promise<Metadata> {
  const tx = await getText();
  const t = tx.research;
  return { title: t.workbench.title };
}

/** RUI-3: the most recent sessions offered for the preview. */
const PICKER_SESSIONS = 50;

/** `/[org]/research/experiments/[id]`: the workbench (RUI-1..5). */
export default async function ExperimentPage({
  params,
  searchParams,
}: {
  params: Promise<{ org: string; id: string }>;
  searchParams: Promise<Record<string, string | string[] | undefined>>;
}) {
  const { org, id } = await params;
  // WP-X.11 "Start run from this dataset": `?dataset=<id>` opens the run dialog on that dataset.
  const query = await searchParams;
  const rawDataset = Array.isArray(query.dataset) ? query.dataset[0] : query.dataset;
  const role = await currentRole();
  const api = await authedApi();
  const published = (kind: ConfigDocumentKind) =>
    loadOrNotFound(() => api.listConfigDocuments(kind, { status: "published", limit: 1 })).then((p) => p.items[0]);
  const [experiment, profile, baselines, catalog, sessionPage, cohorts, datasets] = await Promise.all([
    loadOrNotFound(() => api.getExperiment(id)),
    published("scoring_profile"),
    published("normative_baselines"),
    published("stimulus_catalog"),
    api.listSessions(org, { limit: PICKER_SESSIONS }).then((p) => p.items).catch(() => [] as SessionListItem[]),
    api.listCohorts(org, { limit: 100 }).then((p) => p.items.filter((c) => !c.archived_at)).catch(() => []),
    api
      .listDatasets(org, { status: "frozen", limit: 100 })
      .then((p) => p.items.map(({ id: dsId, name, member_count, content_hash, split }) => ({ id: dsId, name, member_count, content_hash, split })))
      .catch(() => []),
  ]);
  const initialDatasetId = rawDataset && datasets.some((d) => d.id === rawDataset) ? rawDataset : undefined;
  const sessions = pickerSessions(sessionPage);
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
        datasets={datasets}
        {...(initialDatasetId ? { initialDatasetId } : {})}
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
