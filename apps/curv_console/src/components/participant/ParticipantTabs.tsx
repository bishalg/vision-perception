"use client";

import Link from "next/link";
import { useText } from "@/lib/i18n/provider";
import type { WorkspaceTab } from "@/messages/participant";

export { parseWorkspaceTab } from "./workspaceTab";

export function ParticipantTabs({
  basePath,
  active,
  tabs,
}: {
  basePath: string;
  active: WorkspaceTab;
  tabs: readonly WorkspaceTab[];
}) {
  const t = useText().participant;
  return (
    <nav className="tabs no-print" aria-label={t.tabsLabel}>
      {tabs.map((id) => (
        <Link
          key={id}
          href={`${basePath}?tab=${id}`}
          aria-current={id === active ? "page" : undefined}
          className={id === active ? "tab active" : "tab"}
        >
          {t.tabs[id]}
        </Link>
      ))}
    </nav>
  );
}
