import { en } from "@/messages/en";
import type { AttentionFlag } from "@/lib/api";

export function FlagBadge({ flag }: { flag: AttentionFlag }) {
  // Text label always present: colour is never the only signal (WCAG).
  return <span className={`badge badge-${flag}`}>{en.flags[flag]}</span>;
}
