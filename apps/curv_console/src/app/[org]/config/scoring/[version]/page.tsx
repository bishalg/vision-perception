import type { Metadata } from "next";
import { DocVersionPage } from "@/components/config/DocVersionPage";
import { configText } from "@/messages/config";

export const metadata: Metadata = { title: configText.scoring.title };

export default async function ScoringVersionPage({
  params,
  searchParams,
}: {
  params: Promise<{ org: string; version: string }>;
  searchParams: Promise<Record<string, string | string[] | undefined>>;
}) {
  const { org, version } = await params;
  const query = await searchParams;
  return <DocVersionPage org={org} kind="scoring_profile" version={version} searchParams={query} />;
}
