"use client";

import Link from "next/link";
import type { UserProfile } from "@/lib/api";
import { formatDateTime } from "@/lib/format";
import { useText } from "@/lib/i18n/provider";
import { shouldNudgeTotp } from "@/lib/permissions";

/**
 * WP-C.15 / C.17: a reminder for users whose role requires two-step sign-in (`totp_enforcement` `grace`) and who have
 * not turned it on, with the deadline when the backend has set one. Renders nothing otherwise.
 */
export function TotpNudge({ org, profile }: { org: string; profile: UserProfile | null | undefined }) {
  const t = useText().security.security;
  if (!profile || !shouldNudgeTotp(profile)) return null;
  const by = profile.totp_required_by;
  return (
    <p className="panel" role="note" data-testid="totp-nudge">
      {t.nudge.text}
      {by ? ` ${t.nudge.deadline(formatDateTime(by))}` : ""}{" "}
      <Link href={`/${encodeURIComponent(org)}/settings/security`}>{t.nudge.link}</Link>
    </p>
  );
}
