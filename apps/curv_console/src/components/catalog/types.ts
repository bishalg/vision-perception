import { catalogErrorMessage } from "@/messages/catalog";

/** Failure of a catalog server action: the problem slug, status, detail and the field reasons as sent. */
export type CatalogFailure = {
  ok: false;
  slug: string | null;
  status: number;
  detail?: string;
  params?: { name: string; reason: string }[];
};

export type CatalogResult<T = object> = ({ ok: true } & T) | CatalogFailure;

/** `errs` is `t.catalog.errors` of the caller's locale (English when omitted). */
export function catalogFailureText(f: CatalogFailure, errs?: Record<string, string>): string {
  const base = catalogErrorMessage(f.slug, f.status, errs);
  return f.detail && f.slug !== "validation-failed" ? `${base} (${f.detail})` : base;
}
