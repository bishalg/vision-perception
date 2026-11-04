"use client";

import type { AttentionFlag } from "@/lib/api";
import { flagAssessmentText } from "@/lib/flagAssessment";
import { useText } from "@/lib/i18n/provider";
import { enumLabel } from "@/messages/domain";

/**
 * `assessment` (optional): the headline the device sent with the flag; it is shown in the user's language when it is
 * one of the five known texts, and as sent otherwise (I18N-4, I18N-18).
 */
export function FlagBadge({
  flag,
  assessment,
}: {
  flag: AttentionFlag;
  assessment?: { headline: string; detail?: string | null };
}) {
  const t = useText();
  // Text label always present: colour is never the only signal (WCAG).
  const badge = <span className={`badge badge-${flag}`}>{enumLabel(t.locale, "attention_flag", flag)}</span>;
  if (!assessment) return badge;
  return (
    <>
      {badge} <span className="muted">{flagAssessmentText(t.locale, assessment).headline}</span>
    </>
  );
}
