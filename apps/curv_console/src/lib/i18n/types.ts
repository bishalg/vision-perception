/** Console locales (I18N-14). `hi` is a native-app/catalogue locale only. */
export const LOCALES = ["en", "ne"] as const;
export type Locale = (typeof LOCALES)[number];
export const DEFAULT_LOCALE: Locale = "en";

export function isLocale(value: unknown): value is Locale {
  return value === "en" || value === "ne";
}

/**
 * The type of the `ne` tree: string literals widen to `string`, function signatures and everything else are kept, so
 * a missing or extra key is a compile error while the Nepali text itself stays free (I18N-15).
 */
export type Widen<T> = T extends string
  ? string
  : T extends (...args: never[]) => unknown
    ? T
    : T extends readonly (infer U)[]
      ? readonly Widen<U>[]
      : T extends object
        ? { -readonly [K in keyof T]: Widen<T[K]> }
        : T;

/** `<file>Messages`: one tree per locale. */
export interface Localised<T> {
  en: T;
  ne: Widen<T>;
}
