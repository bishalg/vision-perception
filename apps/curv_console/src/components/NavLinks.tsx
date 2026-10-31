"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";

/** Main nav; marks the current section with aria-current. */
export function NavLinks({ links, label }: { links: { href: string; text: string }[]; label: string }) {
  const pathname = usePathname() ?? "";
  return (
    <nav aria-label={label} className="mainnav">
      {links.map((l) => {
        const active = pathname === l.href || pathname.startsWith(`${l.href}/`);
        return (
          <Link key={l.href} href={l.href} aria-current={active ? "page" : undefined} className={active ? "active" : undefined}>
            {l.text}
          </Link>
        );
      })}
    </nav>
  );
}
