"use client";

import { usePathname } from "next/navigation";
import { useEffect, useState, type ReactNode } from "react";
import { activeNav, type Division } from "@/lib/nav";
import { AccountMenu } from "./AccountMenu";
import { Breadcrumb } from "./Breadcrumb";
import { OrgSwitcher } from "./OrgSwitcher";
import { SideNav } from "./SideNav";
import { TopBar } from "./TopBar";
import s from "./shell.module.css";

const DRAWER_ID = "sidenav";

/**
 * The shell's one props contract (WP-C.23). Fed only by `navTree()` and `paths.ts` in the org layout, the only caller,
 * so the shell can be swapped (e.g. for a design system's UI shell) without touching any page.
 */
export interface AppShellProps {
  /** `navTree(base, role, t)`: the visible divisions and their sidebar items. */
  tree: Division[];
  /** `orgPaths(base).account.security`: the Account menu's first entry. */
  accountSecurityHref: string;
  /** The compact org switcher: the current org, the orgs to offer, and whether any id may be typed (superadmin). */
  orgs: { current: string; list: { id: string; name: string }[]; allowAnyId: boolean };
  children: ReactNode;
}

/**
 * Spec 03 §3a: top bar (brand, org switcher, divisions, Menu, Account), sidebar, breadcrumb and the page. CSS grid
 * `sidebar 240px | content` (N-3); the active division and item come from the client pathname (the layout persists
 * across navigations).
 */
export function AppShell({ tree, accountSecurityHref, orgs, children }: AppShellProps) {
  const pathname = usePathname() ?? "";
  const { division, item } = activeNav(tree, pathname);
  const [open, setOpen] = useState(false);
  // A navigation closes the drawer (state adjusted during render on a pathname change, not in an effect).
  const [seenPath, setSeenPath] = useState(pathname);
  if (seenPath !== pathname) {
    setSeenPath(pathname);
    setOpen(false);
  }
  const close = () => {
    setOpen(false);
    document.querySelector<HTMLElement>('[data-shell="menu"]')?.focus();
  };
  // The drawer covers the page: keep the page from scrolling behind it.
  useEffect(() => {
    document.body.classList.toggle("drawer-open", open);
    return () => document.body.classList.remove("drawer-open");
  }, [open]);
  return (
    <div className={s.shell} data-sidebar={division ? "on" : "off"}>
      <TopBar
        tree={tree}
        division={division}
        orgSwitcher={<OrgSwitcher current={orgs.current} orgs={orgs.list} allowAnyId={orgs.allowAnyId} />}
        account={<AccountMenu securityHref={accountSecurityHref} />}
        menuOpen={open}
        onMenu={() => setOpen((o) => !o)}
        drawerId={DRAWER_ID}
      />
      <div className={s.body}>
        <SideNav id={DRAWER_ID} tree={tree} division={division} item={item} open={open} onClose={close} />
        <main className={s.content}>
          <Breadcrumb pathname={pathname} division={division} item={item} />
          {children}
        </main>
      </div>
    </div>
  );
}
