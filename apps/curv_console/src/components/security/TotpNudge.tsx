import Link from "next/link";
import type { Role } from "@/lib/api";
import { shouldNudgeTotp } from "@/lib/permissions";
import { securityText } from "@/messages/security";

/** WP-C.15: a reminder for privileged roles that have not turned on two-step sign-in. Renders nothing otherwise. */
export function TotpNudge({ org, role, totpEnrolled }: { org: string; role: Role | undefined; totpEnrolled: boolean | null | undefined }) {
  if (!shouldNudgeTotp(role, totpEnrolled)) return null;
  return (
    <p className="panel" role="note" data-testid="totp-nudge">
      {securityText.nudge.text}{" "}
      <Link href={`/${encodeURIComponent(org)}/settings/security`}>{securityText.nudge.link}</Link>
    </p>
  );
}
