import type { ReactNode } from "react";
import { currentRole } from "@/lib/auth/server";
import { getText } from "@/lib/i18n/server";
import { can } from "@/lib/permissions";

/**
 * The research area (RUI-1): a role without `analysis.read` sees the denial; everyone else gets the PRV-6 sentence on
 * every page. WP-C.23: the sub-navigation (experiments, cohorts, runs) lives in the shell's sidebar now. The backend
 * authorises every request; this only hides what a role cannot use.
 */
export default async function ResearchLayout({ children }: { children: ReactNode }) {
  const tx = await getText();
  const t = tx.research;
  if (!can(await currentRole(), "analysis.read")) {
    return (
      <>
        <h1>{t.title}</h1>
        <p className="muted">{t.experiments.denied}</p>
      </>
    );
  }
  return (
    <>
      <p className="muted">{t.disclaimer}</p>
      {children}
    </>
  );
}
