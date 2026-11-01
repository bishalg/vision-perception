import type { Metadata } from "next";
import { DocVersionsPage } from "@/components/config/DocVersions";
import { currentRole } from "@/lib/auth/server";
import { configText } from "@/messages/config";

export const metadata: Metadata = { title: configText.scoring.title };

export default async function ScoringPage({
  params,
  searchParams,
}: {
  params: Promise<{ org: string }>;
  searchParams: Promise<Record<string, string | string[] | undefined>>;
}) {
  const { org } = await params;
  const [role, query] = [await currentRole(), await searchParams];
  return (
    <DocVersionsPage
      org={org}
      kind="scoring_profile"
      segment="scoring"
      title={configText.scoring.title}
      intro={configText.scoring.intro}
      role={role}
      searchParams={query}
    />
  );
}
