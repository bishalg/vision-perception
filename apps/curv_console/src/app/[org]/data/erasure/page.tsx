import type { Metadata } from "next";
import Link from "next/link";
import { ErasureTable } from "@/components/privacy/ErasureTable";
import { authedApi, currentRole } from "@/lib/auth/server";
import { parseCursor, toSearch } from "@/lib/listQuery";
import { loadOrNotFound } from "@/lib/loaders";
import { can } from "@/lib/permissions";
import { getText } from "@/lib/i18n/server";
import { pathsFor } from "@/lib/paths";

export async function generateMetadata(): Promise<Metadata> {
  return { title: (await getText()).privacy.erasure.title };
}

export default async function ErasurePage({
  params,
  searchParams,
}: {
  params: Promise<{ org: string }>;
  searchParams: Promise<Record<string, string | string[] | undefined>>;
}) {
  const { org } = await params;
  const text = (await getText()).privacy;
  const t = text.erasure;
  if (!can(await currentRole(), "participant.erase")) {
    return (
      <>
        <h1>{t.title}</h1>
        <p className="muted">{text.denied}</p>
      </>
    );
  }
  const cursor = parseCursor(await searchParams);
  const api = await authedApi();
  const page = await loadOrNotFound(() => api.listErasureRequests(org, { cursor, limit: 50 }));
  const base = pathsFor(org).data.erasure;
  return (
    <>
      <h1>{t.title}</h1>
      <p className="muted">{t.intro}</p>
      <p className="muted" data-testid="held-explainer">{t.heldExplain}</p>
      <ErasureTable items={page.items} />
      <p className="muted">{t.showing(page.items.length)}</p>
      <nav className="pagination" aria-label={text.pagination}>
        {cursor ? <Link href={base}>{t.first}</Link> : null}
        {page.next_cursor ? (
          <Link rel="next" href={`${base}${toSearch({ cursor: page.next_cursor })}`}>{t.next}</Link>
        ) : null}
      </nav>
    </>
  );
}
