import type { Metadata } from "next";
import Link from "next/link";
import { DatasetForm } from "@/components/research/DatasetForm";
import { authedApi, currentRole } from "@/lib/auth/server";
import { getText } from "@/lib/i18n/server";
import { loadOrNotFound } from "@/lib/loaders";
import { pathsFor } from "@/lib/paths";
import { can } from "@/lib/permissions";
import { createDatasetAction } from "../actions";

export async function generateMetadata(): Promise<Metadata> {
  return { title: (await getText()).research.datasets.newTitle };
}

/** `/[org]/research/datasets/new`: freeze a dataset from a saved cohort (WP-X.11, R-7). */
export default async function NewDatasetPage({ params }: { params: Promise<{ org: string }> }) {
  const tx = await getText();
  const t = tx.research.datasets;
  const { org } = await params;
  const back = pathsFor(org).research.datasets;
  const allowed = can(await currentRole(), "experiment.write");
  const cohorts = allowed
    ? (await loadOrNotFound(() => (async () => (await authedApi()).listCohorts(org, { limit: 100 }))())).items.filter((c) => !c.archived_at)
    : [];
  return (
    <>
      <p><Link href={back}>{t.allDatasets}</Link></p>
      <h1>{t.newTitle}</h1>
      {allowed ? (
        <>
          <p className="muted">{t.newIntro}</p>
          <DatasetForm orgId={org} cohorts={cohorts.map((c) => ({ id: c.id, name: c.name }))} create={createDatasetAction.bind(null, org)} />
        </>
      ) : (
        <p className="muted">{t.readOnly}</p>
      )}
    </>
  );
}
