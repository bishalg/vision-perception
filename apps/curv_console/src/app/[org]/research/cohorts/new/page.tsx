import type { Metadata } from "next";
import Link from "next/link";
import { CohortForm } from "@/components/research/CohortForm";
import { currentRole } from "@/lib/auth/server";
import { getText } from "@/lib/i18n/server";
import { can } from "@/lib/permissions";
import { previewCohortAction } from "../../actions";
import { createCohortAction } from "../actions";


export async function generateMetadata(): Promise<Metadata> {
  const tx = await getText();
  const t = tx.research.cohorts;
  return { title: t.newTitle };
}

export default async function NewCohortPage({ params }: { params: Promise<{ org: string }> }) {
  const tx = await getText();
  const t = tx.research.cohorts;
  const { org } = await params;
  const back = `/${encodeURIComponent(org)}/research/cohorts`;
  const allowed = can(await currentRole(), "analysis.run");
  return (
    <>
      <p><Link href={back}>{t.allCohorts}</Link></p>
      <h1>{t.newTitle}</h1>
      {allowed ? (
        <>
          <p className="muted">{t.newIntro}</p>
          <CohortForm orgId={org} create={createCohortAction.bind(null, org)} preview={previewCohortAction.bind(null, org)} />
        </>
      ) : (
        <p className="muted">{t.readOnly}</p>
      )}
    </>
  );
}
