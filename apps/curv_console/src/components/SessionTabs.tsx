import Link from "next/link";
import { en } from "@/messages/en";

export const TABS = ["clips", "quality", "report"] as const;
export type Tab = (typeof TABS)[number];

export function parseTab(value: string | string[] | undefined): Tab {
  const v = Array.isArray(value) ? value[0] : value;
  return (TABS as readonly string[]).includes(v ?? "") ? (v as Tab) : "clips";
}

export function SessionTabs({ basePath, active }: { basePath: string; active: Tab }) {
  return (
    <nav className="tabs" aria-label="Session sections">
      {TABS.map((t) => (
        <Link
          key={t}
          href={`${basePath}?tab=${t}`}
          aria-current={t === active ? "page" : undefined}
          className={t === active ? "tab active" : "tab"}
        >
          {en.detail.tabs[t]}
        </Link>
      ))}
    </nav>
  );
}
