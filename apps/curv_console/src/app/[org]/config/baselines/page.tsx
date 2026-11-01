import type { Metadata } from "next";
import { DocVersionsPage } from "@/components/config/DocVersions";
import { currentRole } from "@/lib/auth/server";
import { configText } from "@/messages/config";

export const metadata: Metadata = { title: configText.baselines.title };

export default async function BaselinesPage({
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
      kind="normative_baselines"
      segment="baselines"
      title={configText.baselines.title}
      intro={configText.baselines.intro}
      role={role}
      searchParams={query}
    />
  );
}
