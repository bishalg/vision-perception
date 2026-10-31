import type { ReactNode } from "react";
import { logoutAction } from "@/app/login/actions";
import { NavLinks } from "@/components/NavLinks";
import { OrgSwitcher } from "@/components/OrgSwitcher";
import { authedApi, currentRole, homeOrg, recentOrgs } from "@/lib/auth/server";
import { loadOrNotFound } from "@/lib/loaders";
import { can } from "@/lib/permissions";
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
  const role = await currentRole();
  // Switcher entries: this org, the home org and recently visited ones the caller can still read.
  const home = await homeOrg();
  const others = [...new Set([home, ...(await recentOrgs())])].filter((id): id is string => !!id && id !== org);
  const resolved = await Promise.allSettled(others.map((id) => api.getOrg(id)));
  const orgs = [
    { id: orgInfo.id, name: orgInfo.name },
    ...resolved.flatMap((r) => (r.status === "fulfilled" ? [{ id: r.value.id, name: r.value.name }] : [])),
  ];
  const base = `/${encodeURIComponent(org)}`;
  const links = [
    { href: `${base}/overview`, text: en.nav.overview },
    { href: `${base}/sessions`, text: en.nav.sessions },
    { href: `${base}/review`, text: en.nav.review },
    ...(can(role, "device.read") ? [{ href: `${base}/devices`, text: en.nav.devices }] : []),
  ];
  return (
    <div className="shell">
      <header className="topbar">
        <span className="brand">{en.appName}</span>
        <OrgSwitcher current={orgInfo.id} orgs={orgs} allowAnyId={role === "superadmin"} />
        <NavLinks links={links} label={en.nav.main} />
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
