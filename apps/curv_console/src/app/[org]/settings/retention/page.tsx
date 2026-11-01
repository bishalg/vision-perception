import type { Metadata } from "next";
import { RetentionPanel } from "@/components/privacy/RetentionPanel";
import { currentRole } from "@/lib/auth/server";
import { can } from "@/lib/permissions";
import { privacyText } from "@/messages/privacy";
import { pollSweepAction, startSweepAction } from "../../privacy-actions";

const t = privacyText.retention;

export const metadata: Metadata = { title: t.title };

export default async function RetentionPage({ params }: { params: Promise<{ org: string }> }) {
  const { org } = await params;
  if (!can(await currentRole(), "participant.erase")) {
    return (
      <>
        <h1>{t.title}</h1>
        <p className="muted">{privacyText.denied}</p>
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
