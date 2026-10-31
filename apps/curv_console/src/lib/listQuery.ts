/** URL state for the review and devices lists. Only opaque ids and enum values; unknown values are dropped. */
import { REVIEW_REASONS, REVIEW_STATUSES, type ReviewListQuery } from "@/lib/api";

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

export function toSearch(params: Record<string, string | undefined>): string {
  const p = new URLSearchParams();
  for (const [k, v] of Object.entries(params)) if (v) p.set(k, v);
  const s = p.toString();
  return s ? `?${s}` : "";
}
