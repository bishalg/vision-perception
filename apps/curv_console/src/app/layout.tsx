import type { Metadata } from "next";
import type { ReactNode } from "react";
import { connection } from "next/server";
import { en } from "@/messages/en";
import "./globals.css";

export const metadata: Metadata = {
  title: { default: en.appName, template: `%s | ${en.appName}` },
  robots: { index: false, follow: false },
};

/**
 * CSP-1: the nonce is per request, so no page may be prerendered to static HTML (its scripts would carry no nonce
 * and `'strict-dynamic'` would block them). `connection()` makes every route under this layout, including the
 * built-in `/_not-found`, render per request.
 */
export default async function RootLayout({ children }: { children: ReactNode }) {
  await connection();
  return (
    <html lang="en">
      <body>{children}</body>
    </html>
  );
}
