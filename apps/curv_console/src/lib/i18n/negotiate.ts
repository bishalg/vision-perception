import { DEFAULT_LOCALE, type Locale } from "./types";

/**
 * Best `Accept-Language` match (I18N-12): RFC 9110 weighted list, `q=0` excluded, primary subtag match (`ne`, `ne-NP`
 * -> `ne`; `en-*` -> `en`; anything else ignored), ties keep header order. `null` when nothing matches.
 */
export function matchAcceptLanguage(header: string | null | undefined): Locale | null {
  if (!header) return null;
  let best: { locale: Locale; q: number } | null = null;
  for (const part of header.split(",")) {
    const [tag = "", ...params] = part.trim().split(";");
    const primary = tag.trim().toLowerCase().split("-")[0];
    if (primary !== "en" && primary !== "ne") continue;
    let q = 1;
    for (const p of params) {
      if (!/^\s*q\s*=/i.test(p)) continue;
      const m = /^\s*q\s*=\s*([0-9]+(?:\.[0-9]+)?)\s*$/i.exec(p);
      q = m ? Number(m[1]) : Number.NaN; // an unparsable weight drops the entry
    }
    if (!Number.isFinite(q) || q <= 0 || q > 1) continue;
    if (!best || q > best.q) best = { locale: primary, q };
  }
  return best?.locale ?? null;
}

/** user > org > Accept-Language > en. Mirrors `asd_contract::i18n::negotiate` (shared fixture). */
export function negotiate(user: Locale | null, org: Locale | null, acceptLanguage: string | null | undefined): Locale {
  return user ?? org ?? matchAcceptLanguage(acceptLanguage) ?? DEFAULT_LOCALE;
}
