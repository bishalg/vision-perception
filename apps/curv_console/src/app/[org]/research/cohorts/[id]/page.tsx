import type { Metadata } from "next";
import Link from "next/link";
import { notFound } from "next/navigation";
import { ArchiveCohortButton } from "@/components/research/ArchiveCohortButton";
import { CohortPreview } from "@/components/research/CohortPreview";
import { authedApi, currentRole } from "@/lib/auth/server";
import { formatDateTime } from "@/lib/format";
import { getText } from "@/lib/i18n/server";
import { loadOrNotFound } from "@/lib/loaders";
import { can } from "@/lib/permissions";
import { describeCohort } from "@/lib/research/cohort";
import { archiveCohortAction } from "../actions";
import { pathsFor } from "@/lib/paths";


export async function generateMetadata(): Promise<Metadata> {
  const tx = await getText();
  const t = tx.research.cohorts;
  return { title: t.title };
}

/** One cohort: its selection, its definition hash and the counts-only preview (suppressed cells show "n < k"). */
export default async function CohortPage({ params }: { params: Promise<{ org: string; id: string }> }) {
  const tx = await getText();
  const t = tx.research.cohorts;
  const { org, id } = await params;
  const api = await authedApi();
  const cohort = await loadOrNotFound(() => api.getCohort(id));
  if (cohort.org_id !== org) notFound();
  const preview = await loadOrNotFound(() => api.previewCohort(org, cohort.definition));
  const canWrite = can(await currentRole(), "analysis.run");
  return (
    <>
      <p><Link href={pathsFor(org).research.cohorts}>{t.allCohorts}</Link></p>
      <h1>{cohort.name}</h1>
      <section className="panel stack" aria-label={t.definition}>
        <h2>{t.definition}</h2>
        <ul>{describeCohort(cohort.definition).map((line) => <li key={line}>{line}</li>)}</ul>
        <p className="muted">{t.definitionHash}: <code>{cohort.definition_hash}</code></p>
        <p className="muted">{formatDateTime(cohort.created_at)}</p>
        {cohort.archived_at ? <p className="muted">{t.archivedNote}</p> : null}
      </section>
      <section className="panel stack" aria-label={t.previewTitle}>
        <h2>{t.previewTitle}</h2>
        <CohortPreview preview={preview} text={tx.research} />
      </section>
      {canWrite && !cohort.archived_at ? <ArchiveCohortButton archive={archiveCohortAction.bind(null, org, id)} /> : null}
    </>
  );
}
