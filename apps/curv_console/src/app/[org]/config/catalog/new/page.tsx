import type { Metadata } from "next";
import Link from "next/link";
import { CatalogEditor } from "@/components/catalog/CatalogEditor";
import { authedApi, currentRole } from "@/lib/auth/server";
import { emptyCatalog, parseCatalog } from "@/lib/catalog";
import { getText } from "@/lib/i18n/server";
import { loadOrNotFound } from "@/lib/loaders";
import { can } from "@/lib/permissions";
import { createCatalogDraftAction } from "../actions";


export async function generateMetadata(): Promise<Metadata> {
  const tx = await getText();
  const t = tx.catalog;
  return { title: t.editor.title };
}

/** `?from=<version>` starts from that version's body; without it, from the published version (else empty). */
export default async function NewCatalogDraftPage({
  params,
  searchParams,
}: {
  params: Promise<{ org: string }>;
  searchParams: Promise<Record<string, string | string[] | undefined>>;
}) {
  const tx = await getText();
  const t = tx.catalog;
  const { org } = await params;
  const base = `/${encodeURIComponent(org)}/config/catalog`;
  const role = await currentRole();
  if (!can(role, "config.manage")) {
    return (
      <>
        <p><Link href={base}>{t.back}</Link></p>
        <h1>{t.editor.title}</h1>
        <p className="muted">{t.editor.denied}</p>
      </>
    );
  }
  const raw = await searchParams;
  const from = (Array.isArray(raw.from) ? raw.from[0] : raw.from)?.slice(0, 64);
  const api = await authedApi();
  const source = from
    ? await loadOrNotFound(() => api.getConfigDocument("stimulus_catalog", from))
    : (await loadOrNotFound(() => api.listConfigDocuments("stimulus_catalog", { status: "published", limit: 1 }))).items[0];
  const initial = (source && parseCatalog(source.body)) || emptyCatalog();
  return (
    <>
      <p><Link href={base}>{t.back}</Link></p>
      <h1>{t.editor.title}</h1>
      <CatalogEditor
        orgId={org}
        initial={initial}
        fromVersion={source && parseCatalog(source.body) ? source.version : null}
        save={createCatalogDraftAction.bind(null, org)}
      />
    </>
  );
}
