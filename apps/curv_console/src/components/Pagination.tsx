import Link from "next/link";
import { en } from "@/messages/en";
import type { SessionListQuery } from "@/lib/api";
import { serializeSessionQuery } from "@/lib/sessionQuery";

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
  const base = `/${encodeURIComponent(orgId)}/sessions`;
  const { cursor, ...filters } = query;
  return (
    <nav className="pagination" aria-label="Pagination">
      <span className="muted">{en.sessions.showing(shown, total)}</span>
      {cursor ? <Link href={`${base}${serializeSessionQuery(filters)}`}>{en.sessions.first}</Link> : null}
      {nextCursor ? (
        <Link rel="next" href={`${base}${serializeSessionQuery({ ...filters, cursor: nextCursor })}`}>
          {en.sessions.next}
        </Link>
      ) : null}
    </nav>
  );
}
