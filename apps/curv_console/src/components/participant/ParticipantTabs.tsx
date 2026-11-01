import Link from "next/link";
import { TAB_IDS, participantText, type WorkspaceTab } from "@/messages/participant";

export function parseWorkspaceTab(value: string | string[] | undefined, allowed: readonly WorkspaceTab[] = TAB_IDS): WorkspaceTab {
  const v = Array.isArray(value) ? value[0] : value;
  return allowed.includes(v as WorkspaceTab) ? (v as WorkspaceTab) : "timeline";
}

export function ParticipantTabs({
  basePath,
  active,
  tabs,
}: {
  basePath: string;
  active: WorkspaceTab;
  tabs: readonly WorkspaceTab[];
}) {
  return (
    <nav className="tabs no-print" aria-label={participantText.tabsLabel}>
      {tabs.map((id) => (
        <Link
          key={id}
          href={`${basePath}?tab=${id}`}
          aria-current={id === active ? "page" : undefined}
          className={id === active ? "tab active" : "tab"}
        >
          {participantText.tabs[id]}
        </Link>
      ))}
    </nav>
  );
}
