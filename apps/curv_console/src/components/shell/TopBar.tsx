"use client";

import Link from "next/link";
import type { ReactNode } from "react";
import { useText } from "@/lib/i18n/provider";
import type { Division } from "@/lib/nav";
import s from "./shell.module.css";

/** N-1/N-3: brand, compact org switcher, the five divisions, the drawer's Menu button (below 1024 px), Account. */
export function TopBar({
  tree,
  division,
  orgSwitcher,
  account,
  menuOpen,
  onMenu,
  drawerId,
}: {
  tree: readonly Division[];
  division: Division | undefined;
  orgSwitcher: ReactNode;
  account: ReactNode;
  menuOpen: boolean;
  onMenu: () => void;
  drawerId: string;
}) {
  const t = useText();
  return (
    <header className={s.topbar} data-shell="topbar">
      <button
        type="button"
        className={s.menuButton}
        data-shell="menu"
        aria-expanded={menuOpen}
        aria-controls={drawerId}
        onClick={onMenu}
      >
        {t.app.nav.menu}
      </button>
      <span className="brand">{t.app.appName}</span>
      {orgSwitcher}
      <nav aria-label={t.app.nav.main} className={s.divisions}>
        {tree.map((d) => (
          <Link
            key={d.key}
            href={d.href}
            aria-current={d === division ? "true" : undefined}
            className={d === division ? s.active : undefined}
          >
            {d.text}
          </Link>
        ))}
      </nav>
      {account}
    </header>
  );
}
