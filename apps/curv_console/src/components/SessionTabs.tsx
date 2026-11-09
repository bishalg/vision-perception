import Link from "next/link";

export const TABS = ["clips", "quality", "replay", "report", "clinical"] as const;
export type Tab = (typeof TABS)[number];

export function parseTab(value: string | string[] | undefined): Tab {
  const v = Array.isArray(value) ? value[0] : value;
  return (TABS as readonly string[]).includes(v ?? "") ? (v as Tab) : "clips";
}

/** `labels` and `label` come from the page's message bundle (`t.app.detail.tabs`, `t.app.detail.tabsLabel`). */
export function SessionTabs({
  basePath,
  active,
  hidden = [],
  labels,
  label,
}: {
  basePath: string;
  active: Tab;
  hidden?: readonly Tab[];
  labels: Record<Tab, string>;
  label: string;
}) {
  return (
    <nav className="tabs" aria-label={label}>
      {TABS.filter((t) => !hidden.includes(t)).map((t) => (
        <Link
          key={t}
          href={`${basePath}?tab=${t}`}
          aria-current={t === active ? "page" : undefined}
          className={t === active ? "tab active" : "tab"}
        >
          {labels[t]}
        </Link>
      ))}
    </nav>
  );
}
