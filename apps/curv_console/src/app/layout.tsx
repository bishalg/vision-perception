import type { Metadata } from "next";
import type { ReactNode } from "react";
import { en } from "@/messages/en";
import "./globals.css";

export const metadata: Metadata = {
  title: { default: en.appName, template: `%s | ${en.appName}` },
  robots: { index: false, follow: false },
};

export default function RootLayout({ children }: { children: ReactNode }) {
  return (
    <html lang="en">
      <body>{children}</body>
    </html>
  );
}
