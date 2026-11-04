import type { Metadata } from "next";
import { DocVersionPage } from "@/components/config/DocVersionPage";
import { getText } from "@/lib/i18n/server";

export async function generateMetadata(): Promise<Metadata> {
  const tx = await getText();
  return { title: tx.config.baselines.title };
}

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
