import type { Metadata } from "next";
import Link from "next/link";
import { BaselinesEditor } from "@/components/config/BaselinesEditor";
import { loadDraftContext } from "@/components/config/draftContext";
import type { NormativeBaselineSet } from "@/lib/api/config";
import { currentRole } from "@/lib/auth/server";
import { parseBaselines } from "@/lib/configDocs";
import { can } from "@/lib/permissions";
import { configText } from "@/messages/config";
import { createDocDraftAction, loadSandboxSessionsAction } from "../../docActions";

const t = configText;

export const metadata: Metadata = { title: t.baselines.editorTitle };

const EMPTY: NormativeBaselineSet = { schemaVersion: "1.0.0", status: "draft", provenance: null, ageBrackets: [], videos: {} };

/** `?from=<version>` starts from that version; without it, from the published set (else empty). */
export default async function NewBaselinesDraftPage({
  params,
  searchParams,
}: {
  params: Promise<{ org: string }>;
  searchParams: Promise<Record<string, string | string[] | undefined>>;
}) {
  const { org } = await params;
  const base = `/${encodeURIComponent(org)}/config/baselines`;
  if (!can(await currentRole(), "config.manage")) {
    return (
      <>
        <p><Link href={base}>{t.back}</Link></p>
        <h1>{t.baselines.editorTitle}</h1>
        <p className="muted">{t.editorDenied}</p>
      </>
    );
  }
  const raw = await searchParams;
  const from = (Array.isArray(raw.from) ? raw.from[0] : raw.from)?.slice(0, 64);
  const { source, preview } = await loadDraftContext(org, "normative_baselines", from);
  const initial = source ? parseBaselines(source.body) : null;
  return (
    <>
      <p><Link href={base}>{t.back}</Link></p>
      <h1>{t.baselines.editorTitle}</h1>
      <BaselinesEditor
        orgId={org}
        initial={initial ?? EMPTY}
        fromVersion={initial && source ? source.version : null}
        preview={{ ...preview, load: loadSandboxSessionsAction }}
        save={createDocDraftAction.bind(null, org, "normative_baselines")}
      />
    </>
  );
}
