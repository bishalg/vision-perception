import type { Locale } from "@/lib/i18n/types";
import { dt } from "@/messages/domain";

/** BCP 47 tag for `Intl.DisplayNames` (names, not numbers: no digit override needed). */
const NAME_TAG: Record<Locale, string> = { en: "en", ne: "ne-NP" };

/**
 * The languages the product records, named the same on the server and in every browser: `Intl.DisplayNames` data can
 * differ between Node and a browser build (a hydration mismatch), so only other codes go through it.
 */
const KNOWN: Record<Locale, Record<string, string>> = {
  en: { en: "English", ne: "Nepali", hi: "Hindi" },
  ne: { en: "अंग्रेजी", ne: "नेपाली", hi: "हिन्दी" },
};

/** A participant's language code (`en`, `ne`) as the language's name in the user's locale; the code when unknown. */
export function languageLabel(locale: Locale, code: string): string {
  const known = KNOWN[locale][code];
  if (known) return known;
  try {
    return new Intl.DisplayNames(NAME_TAG[locale], { type: "language" }).of(code) ?? code;
  } catch {
    return code;
  }
}

const BAND = /^\s*(\d+)\s*[–-]\s*(\d+)\s*(?:mo|m)?\s*$/;
const BAND_OPEN = /^\s*(\d+)\s*\+\s*(?:mo|m)?\s*$/;

/**
 * `age_bracket_at_enrol` as recorded by the app ("24–35 mo") formatted from its months with `age_band.range` (Latin
 * digits in every locale, owner question 12). English keeps the recorded text as it is; a value that is not a month
 * range is shown raw.
 */
export function ageBracketLabel(locale: Locale, raw: string): string {
  if (locale === "en") return raw;
  const range = BAND.exec(raw);
  if (range) return dt(locale, "age_band.range", { min: range[1]!, max: range[2]! });
  const open = BAND_OPEN.exec(raw);
  if (open) return dt(locale, "age_band.range_open", { min: open[1]! });
  return raw;
}
