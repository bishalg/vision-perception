import type { Metadata } from "next";
import Link from "next/link";
import { NewExperimentForm } from "@/components/research/NewExperimentForm";
import { authedApi, currentRole } from "@/lib/auth/server";
import { loadOrNotFound } from "@/lib/loaders";
import { can } from "@/lib/permissions";
import { researchText } from "@/messages/research";
import { createExperimentAction } from "../../actions";

const t = researchText.experiments;

export const metadata: Metadata = { title: t.newTitle };

const PAGE = 50;

/** `/[org]/research/experiments/new` (RUI-1): starts from the published scoring profile or an existing experiment. */
export default async function NewExperimentPage({ params }: { params: Promise<{ org: string }> }) {
  const { org } = await params;
  const base = `/${encodeURIComponent(org)}/research/experiments`;
  if (!can(await currentRole(), "experiment.write")) {
    return (
      <>
        <p><Link href={base}>{researchText.workbench.back}</Link></p>
        <h1>{t.newTitle}</h1>
        <p className="muted">{t.readOnly}</p>
      </>
    );
  }
  const api = await authedApi();
  const page = await loadOrNotFound(() => api.listExperiments(org, { limit: PAGE }));
  return (
    <>
      <p><Link href={base}>{researchText.workbench.back}</Link></p>
      <h1>{t.newTitle}</h1>
      <p className="muted">{t.newIntro}</p>
      <NewExperimentForm orgId={org} experiments={page.items.map((e) => ({ id: e.id, name: e.name }))} create={createExperimentAction.bind(null, org)} />
    </>
  );
}
