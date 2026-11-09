import type { Metadata } from "next";
import Link from "next/link";
import { loadDraftContext } from "@/components/config/draftContext";
import { ScoringProfileDraft } from "@/components/config/ScoringProfileEditor";
import { currentRole } from "@/lib/auth/server";
import { parseProfile } from "@/lib/configDocs";
import { getText } from "@/lib/i18n/server";
import { can } from "@/lib/permissions";
import { createDocDraftAction, loadSandboxSessionsAction } from "../../docActions";
import { pathsFor } from "@/lib/paths";


export async function generateMetadata(): Promise<Metadata> {
  const tx = await getText();
  const t = tx.config;
  return { title: t.scoring.editorTitle };
}

/** `?from=<version>` starts from that version; without it, from the published profile (else the core default). */
export default async function NewScoringDraftPage({
  params,
  searchParams,
}: {
  params: Promise<{ org: string }>;
  searchParams: Promise<Record<string, string | string[] | undefined>>;
}) {
  const tx = await getText();
  const t = tx.config;
  const { org } = await params;
  const base = pathsFor(org).config.scoring;
  if (!can(await currentRole(), "config.manage")) {
    return (
      <>
        <p><Link href={base}>{t.back}</Link></p>
        <h1>{t.scoring.editorTitle}</h1>
        <p className="muted">{t.editorDenied}</p>
      </>
    );
  }
  const raw = await searchParams;
  const from = (Array.isArray(raw.from) ? raw.from[0] : raw.from)?.slice(0, 64);
  const { source, preview } = await loadDraftContext(org, "scoring_profile", from);
  const initial = source ? parseProfile(source.body) : null;
  return (
    <>
      <p><Link href={base}>{t.back}</Link></p>
      <h1>{t.scoring.editorTitle}</h1>
      <ScoringProfileDraft
        orgId={org}
        initial={initial}
        fromVersion={initial && source ? source.version : null}
        preview={{ ...preview, load: loadSandboxSessionsAction }}
        save={createDocDraftAction.bind(null, org, "scoring_profile")}
      />
    </>
  );
}
