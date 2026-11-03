import Link from "next/link";
import type { ReactNode } from "react";
import r from "@/components/research/research.module.css";
import { currentRole } from "@/lib/auth/server";
import { can } from "@/lib/permissions";
import { researchText } from "@/messages/research";

const t = researchText;

/**
 * The research area (RUI-1): a role without `analysis.read` sees the denial; everyone else gets the sub-navigation and
 * the PRV-6 sentence on every page. The backend authorises every request; this only hides what a role cannot use.
 */
export default async function ResearchLayout({ children, params }: { children: ReactNode; params: Promise<{ org: string }> }) {
  const { org } = await params;
  if (!can(await currentRole(), "analysis.read")) {
    return (
      <>
        <h1>{t.title}</h1>
        <p className="muted">{t.experiments.denied}</p>
      </>
    );
  }
  const base = `/${encodeURIComponent(org)}/research`;
  return (
    <>
      <nav className={r.subnav} aria-label={t.title}>
        <Link href={base}>{t.title}</Link>
        <Link href={`${base}/experiments`}>{t.experiments.title}</Link>
      </nav>
      <p className="muted">{t.disclaimer}</p>
      {children}
    </>
  );
}
