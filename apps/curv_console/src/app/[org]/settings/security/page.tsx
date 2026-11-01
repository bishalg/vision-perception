import type { Metadata } from "next";
import { TotpNudge } from "@/components/security/TotpNudge";
import { TotpPanel } from "@/components/security/TotpPanel";
import { authedApi, currentRole } from "@/lib/auth/server";
import { securityText } from "@/messages/security";
import { confirmTotpAction, disableTotpAction, enrolTotpAction } from "./actions";

export const metadata: Metadata = { title: securityText.title };

/**
 * WP-C.15: the signed-in user's own second factor (any role). The current state comes from `GET /users/me`
 * (`totp_enrolled`, `totp_pending`); when that read fails the page still offers both setup and turn-off, and the
 * backend answers `totp-already-enrolled` / `totp-not-enrolled` when a step does not apply.
 */
export default async function SecuritySettingsPage({ params }: { params: Promise<{ org: string }> }) {
  const { org } = await params;
  const api = await authedApi();
  const [role, profile] = await Promise.all([currentRole(), api.getMyProfile().catch(() => null)]);
  const enrolled = profile?.totp_enrolled ?? null;
  return (
    <>
      <h1>{securityText.title}</h1>
      <p className="muted">{securityText.intro}</p>
      <TotpNudge org={org} role={role} totpEnrolled={enrolled} />
      {profile?.totp_pending && !enrolled ? <p className="muted">{securityText.status.pending}</p> : null}
      <TotpPanel
        initialEnrolled={enrolled}
        actions={{ enrol: enrolTotpAction, confirm: confirmTotpAction, disable: disableTotpAction }}
      />
    </>
  );
}
