import type { Metadata } from "next";
import { DocVersionPage } from "@/components/config/DocVersionPage";
import { configText } from "@/messages/config";

export const metadata: Metadata = { title: configText.baselines.title };

export default async function BaselinesVersionPage({
  params,
  searchParams,
}: {
  params: Promise<{ org: string; version: string }>;
  searchParams: Promise<Record<string, string | string[] | undefined>>;
}) {
  const { org, version } = await params;
  const query = await searchParams;
  return <DocVersionPage org={org} kind="normative_baselines" version={version} searchParams={query} />;
}
