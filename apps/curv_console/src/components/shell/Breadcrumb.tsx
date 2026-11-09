"use client";

import Link from "next/link";
import { useText } from "@/lib/i18n/provider";
import type { Division, NavItem } from "@/lib/nav";
import s from "./shell.module.css";

const UUID_RE = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

/** The detail part of a path below its sidebar item (`…/sessions/{id}` -> id; `…/new` -> "New"), or null. */
export function detailLabel(pathname: string, item: NavItem | undefined, newText: string): string | null {
  if (!item || pathname === item.href || !pathname.startsWith(`${item.href}/`)) return null;
  const first = pathname.slice(item.href.length + 1).split("/")[0] ?? "";
  if (!first) return null;
  if (first === "new") return newText;
  let text: string;
  try {
    text = decodeURIComponent(first);
  } catch {
    text = first;
  }
  // A full UUID is noise in a breadcrumb: its first block identifies it on the page.
  return UUID_RE.test(text) ? `${text.slice(0, 8)}…` : text;
}

/** N-2: Division › Item › Detail, for the page in the current division. */
export function Breadcrumb({
  pathname,
  division,
  item,
}: {
  pathname: string;
  division: Division | undefined;
  item: NavItem | undefined;
}) {
  const t = useText().app.nav;
  if (!division) return null;
  const detail = detailLabel(pathname, item, t.newItem);
  return (
    <nav aria-label={t.breadcrumb} className={s.breadcrumb}>
      <ol>
        <li>
          <Link href={division.href}>{division.text}</Link>
        </li>
        {item ? (
          <li>
            <Link href={item.href} aria-current={detail ? undefined : "page"}>
              {item.text}
            </Link>
          </li>
        ) : null}
        {detail ? <li aria-current="page">{detail}</li> : null}
      </ol>
    </nav>
  );
}
