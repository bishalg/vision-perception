import type { Metadata } from "next";
import Link from "next/link";
import { CohortForm } from "@/components/research/CohortForm";
import { currentRole } from "@/lib/auth/server";
import { can } from "@/lib/permissions";
import { researchText } from "@/messages/research";
import { previewCohortAction } from "../../actions";
import { createCohortAction } from "../actions";

const t = researchText.cohorts;

export const metadata: Metadata = { title: t.newTitle };

export default async function NewCohortPage({ params }: { params: Promise<{ org: string }> }) {
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
