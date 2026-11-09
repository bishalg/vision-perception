"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import { useEffect, useRef } from "react";
import { logoutAction } from "@/app/login/actions";
import { useText } from "@/lib/i18n/provider";
import s from "./shell.module.css";

/**
 * WP-C.23: the Account menu at the right of the top bar (not a division): Account security for every signed-in user
 * (the TOTP enforcement redirect lands there) and Sign out. A `<details>` disclosure, so it works without JS.
 */
export function AccountMenu({ securityHref }: { securityHref: string }) {
  const t = useText();
  const ref = useRef<HTMLDetailsElement>(null);
  const pathname = usePathname();
  // Close after a navigation (the layout, and with it this menu, persists across client navigations).
  useEffect(() => {
    if (ref.current) ref.current.open = false;
  }, [pathname]);
  const onKeyDown = (e: React.KeyboardEvent<HTMLDetailsElement>) => {
    if (e.key === "Escape" && ref.current?.open) {
      ref.current.open = false;
      ref.current.querySelector("summary")?.focus();
    }
  };
  return (
    <details ref={ref} className={`${s.menu} ${s.account}`} onKeyDown={onKeyDown}>
      <summary>{t.app.nav.account}</summary>
      <div className={s.menuPanel}>
        <Link href={securityHref} aria-current={pathname === securityHref ? "page" : undefined}>
          {t.security.security.nav}
        </Link>
        <form action={logoutAction}>
          <button type="submit" className="link-button">
            {t.app.nav.signOut}
          </button>
        </form>
      </div>
    </details>
  );
}
