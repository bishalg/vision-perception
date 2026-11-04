import type { SessionSummary } from "@/lib/api";
import { messagesFor, type Messages } from "@/lib/i18n/messages";
import { useText } from "@/lib/i18n/provider";

/**
 * SCH-5 (session schema 1.8): "Not scored (reason)" for a session whose on-device scoring failed, `null` for a
 * scored one (1.6 and 1.7 sessions are always scored). Shown instead of every score number.
 */
export function notScoredLabel(
  session: Pick<SessionSummary, "scoring_status" | "unscorable_reason">,
  text: Messages["app"] = messagesFor("en").app,
): string | null {
  const t = text.detail.scoring;
  if (session.scoring_status !== "unscorable") return null;
  return session.unscorable_reason ? t.withReason(t.reasons[session.unscorable_reason]) : t.notScored;
}

/** A score cell: the formatted number, or the "Not scored" label for an unscorable session. */
export function ScoreValue({
  session,
  value,
}: {
  session: Pick<SessionSummary, "scoring_status" | "unscorable_reason">;
  value: string;
}) {
  const label = notScoredLabel(session, useText().app);
  return label ? <span className="muted">{label}</span> : <>{value}</>;
}
