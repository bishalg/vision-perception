import Link from "next/link";
import type { ReactNode } from "react";
import { logoutAction } from "@/app/login/actions";
import { authedApi } from "@/lib/auth/server";
import { loadOrNotFound } from "@/lib/loaders";
import { en } from "@/messages/en";

export default async function OrgLayout({
  children,
  params,
}: {
  children: ReactNode;
  params: Promise<{ org: string }>;
}) {
  const { org } = await params;
  const api = await authedApi();
  const orgInfo = await loadOrNotFound(() => api.getOrg(org));
  const base = `/${encodeURIComponent(org)}`;
  return (
    <div className="shell">
      <header className="topbar">
        <span className="brand">{en.appName}</span>
        <span className="org-name">{orgInfo.name}</span>
        <nav aria-label={en.nav.main} className="mainnav">
          <Link href={`${base}/overview`}>{en.nav.overview}</Link>
          <Link href={`${base}/sessions`}>{en.nav.sessions}</Link>
        </nav>
        <form action={logoutAction}>
          <button type="submit" className="link-button">
            {en.nav.signOut}
          </button>
        </form>
      </header>
      <main className="content">{children}</main>
    </div>
  );
}
