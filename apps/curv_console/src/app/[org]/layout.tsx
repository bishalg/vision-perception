import { headers } from "next/headers";
import { redirect } from "next/navigation";
import type { ReactNode } from "react";
import { AppShell } from "@/components/shell/AppShell";
import { enforcementRedirect } from "@/lib/auth/guard";
import { authedApi, currentRole, homeOrg, recentOrgs } from "@/lib/auth/server";
import { loadOrNotFound } from "@/lib/loaders";
import { navTree } from "@/lib/nav";
import { getText } from "@/lib/i18n/server";
import { orgBase, orgPaths } from "@/lib/paths";

export default async function OrgLayout({
  children,
  params,
}: {
  children: ReactNode;
  params: Promise<{ org: string }>;
}) {
  const { org } = await params;
  const t = await getText();
  const api = await authedApi();
  // `GET /users/me` and `GET /orgs/{id}` are the two reads a user past the TOTP deadline may still make (AUTH-13).
  const [orgInfo, profile] = await Promise.all([
    loadOrNotFound(() => api.getOrg(org)),
    api.getMyProfile().catch(() => null),
  ]);
  // Redirect on document loads only. A layout is kept by the client router across navigations, so a layout that
  // throws a redirect during an RSC request (the render a server action such as login triggers for its target, or a
  // client navigation) is cached as a redirect and loops: the client keeps asking for the security page that sits
  // under this same layout. Those requests are covered by the page-level redirect in `loadOrNotFound` instead.
  // Next hides its own `rsc` header from server code, so a document load is recognised by the browser's
  // `Sec-Fetch-Dest: document` (an older browser without it just relies on the page-level redirect).
  const requestHeaders = await headers();
  if (requestHeaders.get("sec-fetch-dest") === "document") {
    const toSecurity = enforcementRedirect(profile, requestHeaders.get("x-pathname"), org);
    if (toSecurity) redirect(toSecurity);
  }
  const role = await currentRole();
  // Switcher entries: this org, the home org and recently visited ones the caller can still read.
  const home = await homeOrg();
  const others = [...new Set([home, ...(await recentOrgs())])].filter((id): id is string => !!id && id !== org);
  const resolved = await Promise.allSettled(others.map((id) => api.getOrg(id)));
  const orgs = [
    { id: orgInfo.id, name: orgInfo.name },
    ...resolved.flatMap((r) => (r.status === "fulfilled" ? [{ id: r.value.id, name: r.value.name }] : [])),
  ];
  const base = orgBase(org);
  return (
    <AppShell
      tree={navTree(base, role, t)}
      accountSecurityHref={orgPaths(base).account.security}
      orgs={{ current: orgInfo.id, list: orgs, allowAnyId: role === "superadmin" }}
    >
      {children}
    </AppShell>
  );
}
