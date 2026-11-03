/** Result type of the research server actions (WP-X.5) and the text of a failure. Plain module: usable by server and client. */
import { researchText, type ResearchMessages } from "@/messages/research";

export type ResearchFailure = {
  ok: false;
  slug: string | null;
  status: number;
  detail?: string;
  /** Field reasons as the server sent them (`config-invalid` carries one per parameter path). */
  params?: { name: string; reason: string }[];
};

export type ResearchResult<T = object> = ({ ok: true } & T) | ResearchFailure;

export function failureText(f: ResearchFailure, text: ResearchMessages = researchText): string {
  const e = text.errors;
  if (f.slug && e[f.slug]) return e[f.slug]!;
  if (f.status === 403) return e.forbidden!;
  if (f.status === 404) return e["not-found"]!;
  return e.fallback!;
}

/** Field reasons keyed by the parameter path, for the registry form. */
export function fieldReasons(params: { name: string; reason: string }[] | undefined): Record<string, string> {
  const out: Record<string, string> = {};
  for (const p of params ?? []) out[p.name] = out[p.name] ? `${out[p.name]}; ${p.reason}` : p.reason;
  return out;
}
