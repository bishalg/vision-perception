/** URL state for the review and devices lists. Only opaque ids and enum values; unknown values are dropped. */
import { REVIEW_REASONS, REVIEW_STATUSES, type AuditQuery, type ReviewListQuery } from "@/lib/api";

type Raw = Record<string, string | string[] | undefined>;

function one(raw: Raw, key: string): string | undefined {
  const v = raw[key];
  const s = Array.isArray(v) ? v[0] : v;
  return s ? s : undefined;
}

/** `status` defaults to `open` (the working queue); `status=all` lists every status. */
export function parseReviewQuery(raw: Raw): ReviewListQuery & { all?: true } {
  const q: ReviewListQuery & { all?: true } = {};
  const status = one(raw, "status");
  if (status === "all") q.all = true;
  else q.status = status && (REVIEW_STATUSES as readonly string[]).includes(status) ? (status as ReviewListQuery["status"]) : "open";
  const reason = one(raw, "reason");
  if (reason && (REVIEW_REASONS as readonly string[]).includes(reason)) q.reason = reason as ReviewListQuery["reason"];
  const cursor = one(raw, "cursor");
  if (cursor) q.cursor = cursor.slice(0, 200);
  return q;
}

export function parseCursor(raw: Raw): string | undefined {
  return one(raw, "cursor")?.slice(0, 200);
}

const AUDIT_FILTER_KEYS = ["actor_id", "action", "target_type", "target_id", "from", "to"] as const;

/** A `YYYY-MM-DD` date means the whole UTC day: start of day for `from`, end of day for `to`. */
function auditTime(v: string, edge: "from" | "to"): string | undefined {
  const day = /^\d{4}-\d{2}-\d{2}$/.test(v);
  const t = Date.parse(day ? `${v}T${edge === "from" ? "00:00:00.000" : "23:59:59.999"}Z` : v);
  return Number.isNaN(t) ? undefined : new Date(t).toISOString();
}

/**
 * URL state of the audit log: the optional filters (free text, trimmed, capped at the backend's 200 characters) and
 * the cursor. `from`/`to` come from date inputs (or an RFC 3339 instant in pagination links); anything that is not a
 * date is dropped rather than sent. The backend validates everything again.
 */
export function parseAuditQuery(raw: Raw): AuditQuery {
  const q: AuditQuery = {};
  for (const k of AUDIT_FILTER_KEYS) {
    const v = one(raw, k)?.trim().slice(0, 200);
    if (!v) continue;
    if (k === "from" || k === "to") {
      const t = auditTime(v, k);
      if (t) q[k] = t;
    } else q[k] = v;
  }
  const cursor = parseCursor(raw);
  if (cursor) q.cursor = cursor;
  return q;
}

/** The filter part of an audit query as URL parameters (no cursor), for the pagination links. */
export function auditFilterParams(q: AuditQuery): Record<string, string | undefined> {
  return Object.fromEntries(AUDIT_FILTER_KEYS.map((k) => [k, q[k]]));
}

export function toSearch(params: Record<string, string | undefined>): string {
  const p = new URLSearchParams();
  for (const [k, v] of Object.entries(params)) if (v) p.set(k, v);
  const s = p.toString();
  return s ? `?${s}` : "";
}
