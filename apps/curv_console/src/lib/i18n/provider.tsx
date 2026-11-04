"use client";
import { createContext, useContext, useMemo, type ReactNode } from "react";
import { messagesFor, type Messages } from "./messages";
import { DEFAULT_LOCALE, type Locale } from "./types";

const LocaleContext = createContext<Locale>(DEFAULT_LOCALE);

/** Root-layout provider (I18N-16). Without a provider (unit tests) the locale is `en`. */
export function LocaleProvider({ locale, children }: { locale: Locale; children: ReactNode }) {
  return <LocaleContext.Provider value={locale}>{children}</LocaleContext.Provider>;
}

export function useLocale(): Locale {
  return useContext(LocaleContext);
}

/** The message bundle for the current locale, for client components. */
export function useText(): Messages {
  const locale = useLocale();
  return useMemo(() => messagesFor(locale), [locale]);
}
