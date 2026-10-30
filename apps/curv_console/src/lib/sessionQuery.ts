/** Parse and serialise the sessions list URL state (filters + cursor). Opaque ids only; no PHI in URLs. */
import { ATTENTION_FLAGS, type AttentionFlag, type SessionListQuery } from "@/lib/api";

type Raw = Record<string, string | string[] | undefined>;

function one(raw: Raw, key: string): string | undefined {
  const v = raw[key];
  const s = Array.isArray(v) ? v[0] : v;
  return s ? s : undefined;
}

const DATE_RE = /^\d{4}-\d{2}-\d{2}$/;

export function parseSessionQuery(raw: Raw): SessionListQuery {
  const q: SessionListQuery = {};
  const flag = one(raw, "flag");
  if (flag && (ATTENTION_FLAGS as readonly string[]).includes(flag)) q.flag = flag as AttentionFlag;
  const quality = one(raw, "quality");
  if (quality) q.quality = quality.slice(0, 40);
  const from = one(raw, "from");
  if (from && DATE_RE.test(from)) q.from = from;
  const to = one(raw, "to");
  if (to && DATE_RE.test(to)) q.to = to;
  const bundle = one(raw, "bundle_version");
  if (bundle) q.bundle_version = bundle.slice(0, 40);
  const reviewed = one(raw, "reviewed");
  if (reviewed === "true") q.reviewed = true;
  if (reviewed === "false") q.reviewed = false;
  const cursor = one(raw, "cursor");
  if (cursor) q.cursor = cursor.slice(0, 200);
  return q;
}

export function serializeSessionQuery(q: SessionListQuery): string {
  const p = new URLSearchParams();
  for (const [k, v] of Object.entries(q)) {
    if (v === undefined || v === "") continue;
    p.set(k, String(v));
  }
  const s = p.toString();
  return s ? `?${s}` : "";
}
