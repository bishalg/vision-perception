"use client";

import Link from "next/link";
import { useEffect, useRef } from "react";
import { useText } from "@/lib/i18n/provider";
import type { Division, NavItem } from "@/lib/nav";
import s from "./shell.module.css";

/** Below this width the sidebar is a drawer (N-3); keep in step with `globals.css`. */
export const DRAWER_QUERY = "(max-width: 1023px)";

const FOCUSABLE = 'a[href], button:not([disabled]), input:not([disabled]), select:not([disabled]), [tabindex]:not([tabindex="-1"])';

/** Focusable elements CSS shows (the drawer's division list is hidden at 640 px and wider). */
function focusables(root: HTMLElement): HTMLElement[] {
  return [...root.querySelectorAll<HTMLElement>(FOCUSABLE)].filter(
    // Without `checkVisibility` (jsdom) every element counts.
    (n) => (n as HTMLElement & { checkVisibility?: () => boolean }).checkVisibility?.() ?? true,
  );
}

/**
 * N-3/N-5: the current division's pages. At 1024 px and wider a static column; below, a drawer opened by the top
 * bar's Menu button that traps focus and closes on Escape (or a click on the backdrop). Below 640 px the drawer also
 * lists the divisions, which the top bar then hides.
 */
export function SideNav({
  id,
  tree,
  division,
  item,
  open,
  onClose,
}: {
  id: string;
  tree: readonly Division[];
  division: Division | undefined;
  item: NavItem | undefined;
  open: boolean;
  onClose: () => void;
}) {
  const t = useText().app.nav;
  const ref = useRef<HTMLDivElement>(null);
  const inDrawer = () => typeof window !== "undefined" && window.matchMedia?.(DRAWER_QUERY).matches === true;

  useEffect(() => {
    if (open && inDrawer() && ref.current) focusables(ref.current)[0]?.focus();
  }, [open]);

  const onKeyDown = (e: React.KeyboardEvent<HTMLDivElement>) => {
    if (!open || !inDrawer()) return;
    if (e.key === "Escape") {
      e.preventDefault();
      onClose();
      return;
    }
    if (e.key !== "Tab" || !ref.current) return;
    const nodes = focusables(ref.current);
    if (!nodes.length) return;
    const first = nodes[0]!;
    const last = nodes[nodes.length - 1]!;
    if (e.shiftKey && document.activeElement === first) {
      e.preventDefault();
      last.focus();
    } else if (!e.shiftKey && document.activeElement === last) {
      e.preventDefault();
      first.focus();
    }
  };

  return (
    <>
      {open ? <div className={s.backdrop} onClick={onClose} aria-hidden="true" /> : null}
      <div id={id} ref={ref} className={s.sidenav} data-shell="sidenav" data-open={open ? "true" : "false"} onKeyDown={onKeyDown}>
        <nav aria-label={t.main} className={s.drawerDivisions}>
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
        {division ? (
          <nav aria-label={division.text}>
            <p className={s.sidenavTitle} aria-hidden="true">
              {division.text}
            </p>
            {division.items.map((i) => (
              <Link
                key={i.href}
                href={i.href}
                aria-current={i === item ? "page" : undefined}
                className={i === item ? s.active : undefined}
              >
                {i.text}
              </Link>
            ))}
          </nav>
        ) : null}
      </div>
    </>
  );
}
