import { DEFAULT_LOCALE, type Locale } from "@/lib/i18n/types";

/**
 * Number and date formatting (I18N-17). Every function takes the locale as its last parameter, default `en`, so
 * existing call sites compile unchanged. Digits are Latin in both locales (owner question 12), the calendar is
 * Gregorian (owner question 13), and unit words come from the message bundle (`t.app.units`).
 */

/** BCP 47 tag used for `Intl`; `ne` forces Latin digits (`-u-nu-latn`). */
export function intlTag(locale: Locale): string {
  return locale === "ne" ? "ne-NP-u-nu-latn" : "en-GB";
}

export function formatDateTime(iso: string): string {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  // No locale parameter: UTC and the fixed form in every locale so server and client render identically (no hydration drift).
  return d.toISOString().slice(0, 16).replace("T", " ") + " UTC";
}

export function formatDuration(sec: number): string {
  const m = Math.floor(sec / 60);
  const s = Math.round(sec % 60);
  return `${m}:${String(s).padStart(2, "0")}`;
}

export function formatNumber(value: number | null | undefined, digits = 1, locale: Locale = DEFAULT_LOCALE): string {
  if (value == null) return "-";
  if (locale === DEFAULT_LOCALE) return value.toFixed(digits);
  return new Intl.NumberFormat(intlTag(locale), {
    minimumFractionDigits: digits,
    maximumFractionDigits: digits,
    useGrouping: false,
  }).format(value);
}

export function formatPercent(value: number | null | undefined, locale: Locale = DEFAULT_LOCALE): string {
  if (value == null) return "-";
  const pct = Math.round(value * 100);
  return `${locale === DEFAULT_LOCALE ? pct : new Intl.NumberFormat(intlTag(locale), { useGrouping: false }).format(pct)}%`;
}

/** The short unit words of `formatAge` (`t.app.units`). */
export interface AgeUnits {
  minutesShort: string;
  hoursShort: string;
  daysShort: string;
}

const EN_UNITS: AgeUnits = { minutesShort: "m", hoursShort: "h", daysShort: "d" };

/** Time since `iso` (queue SLA age): "5m", "2h 30m", "3d 4h"; the unit words follow the locale's bundle. */
export function formatAge(
  iso: string,
  now: number = Date.now(),
  locale: Locale = DEFAULT_LOCALE,
  units: AgeUnits = EN_UNITS,
): string {
  const t = Date.parse(iso);
  if (Number.isNaN(t)) return "-";
  const n = (v: number) => (locale === DEFAULT_LOCALE ? String(v) : new Intl.NumberFormat(intlTag(locale), { useGrouping: false }).format(v));
  const min = Math.max(0, Math.floor((now - t) / 60_000));
  if (min < 60) return `${n(min)}${units.minutesShort}`;
  const h = Math.floor(min / 60);
  if (h < 24) return `${n(h)}${units.hoursShort} ${n(min % 60)}${units.minutesShort}`;
  return `${n(Math.floor(h / 24))}${units.daysShort} ${n(h % 24)}${units.hoursShort}`;
}
