import type { Metadata } from "next";
import { RetentionPanel } from "@/components/privacy/RetentionPanel";
import { currentRole } from "@/lib/auth/server";
import { can } from "@/lib/permissions";
import { getText } from "@/lib/i18n/server";
import { pollSweepAction, startSweepAction } from "../../privacy-actions";

export async function generateMetadata(): Promise<Metadata> {
  return { title: (await getText()).privacy.retention.title };
}

export default async function RetentionPage({ params }: { params: Promise<{ org: string }> }) {
  const { org } = await params;
  const text = (await getText()).privacy;
  const t = text.retention;
  if (!can(await currentRole(), "participant.erase")) {
    return (
      <>
        <h1>{t.title}</h1>
        <p className="muted">{text.denied}</p>
      </>
    );
  }
  return (
    <>
      <h1>{t.title}</h1>
      <RetentionPanel start={startSweepAction.bind(null, org)} poll={pollSweepAction} />
    </>
  );
}
