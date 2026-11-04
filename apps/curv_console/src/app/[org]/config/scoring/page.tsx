import type { Metadata } from "next";
import { DocVersionsPage } from "@/components/config/DocVersions";
import { currentRole } from "@/lib/auth/server";
import { getText } from "@/lib/i18n/server";

export async function generateMetadata(): Promise<Metadata> {
  const tx = await getText();
  return { title: tx.config.scoring.title };
}

export default async function ScoringPage({
  params,
  searchParams,
}: {
  params: Promise<{ org: string }>;
  searchParams: Promise<Record<string, string | string[] | undefined>>;
}) {
  const tx = await getText();
  const { org } = await params;
  const [role, query] = [await currentRole(), await searchParams];
  return (
    <DocVersionsPage
      org={org}
      kind="scoring_profile"
      segment="scoring"
      title={tx.config.scoring.title}
      intro={tx.config.scoring.intro}
      role={role}
      searchParams={query}
    />
  );
}
