"use client";

import Link from "next/link";
import { useText } from "@/lib/i18n/provider";
import type { SessionListQuery } from "@/lib/api";
import { serializeSessionQuery } from "@/lib/sessionQuery";
import { pathsFor } from "@/lib/paths";

export function Pagination({
  orgId,
  query,
  nextCursor,
  shown,
  total,
}: {
  orgId: string;
  query: SessionListQuery;
  nextCursor: string | null | undefined;
  shown: number;
  total: number | null | undefined;
}) {
  const t = useText().app;
  const base = pathsFor(orgId).clinic.sessions;
  const { cursor, ...filters } = query;
  return (
    <nav className="pagination" aria-label={t.common.pagination}>
      <span className="muted">{t.sessions.showing(shown, total)}</span>
      {cursor ? <Link href={`${base}${serializeSessionQuery(filters)}`}>{t.sessions.first}</Link> : null}
      {nextCursor ? (
        <Link rel="next" href={`${base}${serializeSessionQuery({ ...filters, cursor: nextCursor })}`}>
          {t.sessions.next}
        </Link>
      ) : null}
    </nav>
  );
}
