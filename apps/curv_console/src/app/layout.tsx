import type { Metadata } from "next";
import type { ReactNode } from "react";
import { connection } from "next/server";
import localFont from "next/font/local";
import { LocaleProvider } from "@/lib/i18n/provider";
import { getText, resolveLocale } from "@/lib/i18n/server";
import "./globals.css";

/**
 * I18N-21: Noto Sans Devanagari (SIL OFL-1.1, see `fonts/OFL.txt`), self-hosted and subset: no third-party request,
 * CSP `font-src 'self'` unchanged. The Devanagari and Latin subsets are separate files, hence two families; the
 * `:lang(ne)` stack in `globals.css` lists both before the system fonts.
 */
const devanagari = localFont({
  src: "./fonts/NotoSansDevanagari-Devanagari-wght.woff2",
  weight: "100 900",
  variable: "--font-devanagari",
  display: "swap",
  preload: false,
});
const devanagariLatin = localFont({
  src: "./fonts/NotoSansDevanagari-Latin-wght.woff2",
  weight: "100 900",
  variable: "--font-devanagari-latin",
  display: "swap",
  preload: false,
});

export async function generateMetadata(): Promise<Metadata> {
  const t = await getText();
  return {
    title: { default: t.app.appName, template: `%s | ${t.app.appName}` },
    robots: { index: false, follow: false },
  };
}

/**
 * CSP-1: the nonce is per request, so no page may be prerendered to static HTML (its scripts would carry no nonce
 * and `'strict-dynamic'` would block them). `connection()` makes every route under this layout, including the
 * built-in `/_not-found`, render per request.
 */
export default async function RootLayout({ children }: { children: ReactNode }) {
  await connection();
  const locale = await resolveLocale();
  return (
    <html lang={locale === "ne" ? "ne-NP" : "en"} className={`${devanagari.variable} ${devanagariLatin.variable}`}>
      <body>
        <LocaleProvider locale={locale}>{children}</LocaleProvider>
      </body>
    </html>
  );
}
