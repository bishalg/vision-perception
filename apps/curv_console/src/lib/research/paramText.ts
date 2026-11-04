import type { ParamSpec } from "@/lib/api/research";
import type { Locale } from "@/lib/i18n/types";
import { DOMAIN, dt, type DomainKey } from "@/messages/domain";
import { paramRegistry } from "./registry";

export interface ParamText {
  label: string;
  help: string;
}

const ROWS = new Map(paramRegistry.params.map((p) => [p.id, p]));

function lookup(locale: Locale, key: string, fallback: string): string {
  return key in DOMAIN[locale] ? dt(locale, key as DomainKey) : fallback;
}

/**
 * Label and help of a registry row from the domain catalogue, `dt(locale, spec.labelKey)`; a row id is resolved
 * through the bundled registry. The row id itself when the key is missing.
 */
export function paramText(locale: Locale, spec: string | Pick<ParamSpec, "id" | "labelKey" | "helpKey">): ParamText {
  const row = typeof spec === "string" ? ROWS.get(spec) : spec;
  if (!row) return { label: String(spec), help: String(spec) };
  return { label: lookup(locale, row.labelKey, row.id), help: lookup(locale, row.helpKey, row.id) };
}
