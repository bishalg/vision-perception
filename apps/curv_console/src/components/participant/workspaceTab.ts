import { TAB_IDS, type WorkspaceTab } from "@/messages/participant";

export function parseWorkspaceTab(value: string | string[] | undefined, allowed: readonly WorkspaceTab[] = TAB_IDS): WorkspaceTab {
  const v = Array.isArray(value) ? value[0] : value;
  return allowed.includes(v as WorkspaceTab) ? (v as WorkspaceTab) : "timeline";
}
