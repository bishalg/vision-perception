import type { Metadata } from "next";
import { DocVersionsPage } from "@/components/config/DocVersions";
import { currentRole } from "@/lib/auth/server";
import { getText } from "@/lib/i18n/server";

export async function generateMetadata(): Promise<Metadata> {
  const tx = await getText();
  return { title: tx.config.baselines.title };
}

export default async function BaselinesPage({
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
      kind="normative_baselines"
      segment="baselines"
      title={tx.config.baselines.title}
      intro={tx.config.baselines.intro}
      role={role}
      searchParams={query}
    />
  );
}
