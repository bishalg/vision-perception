import type { Metadata } from "next";
import { TotpNudge } from "@/components/security/TotpNudge";
import { TotpPanel } from "@/components/security/TotpPanel";
import { RequiredPanel } from "@/components/security/RequiredPanel";
import { authedApi } from "@/lib/auth/server";
import { securityText } from "@/messages/security";
import { confirmTotpAction, disableTotpAction, enrolTotpAction } from "./actions";

export const metadata: Metadata = { title: securityText.title };

/**
 * WP-C.15: the signed-in user's own second factor (any role). The current state comes from `GET /users/me`
 * (`totp_enrolled`, `totp_pending`); when that read fails the page still offers both setup and turn-off, and the
 * backend answers `totp-already-enrolled` / `totp-not-enrolled` when a step does not apply. WP-C.17: the layout sends
 * users past their deadline here with `?required=1`, which shows the required-state panel until they enrol.
 */
export default async function SecuritySettingsPage({
  params,
  searchParams,
}: {
  params: Promise<{ org: string }>;
  searchParams: Promise<{ required?: string }>;
}) {
  const { org } = await params;
  const { required } = await searchParams;
  const api = await authedApi();
  const profile = await api.getMyProfile().catch(() => null);
  const enrolled = profile?.totp_enrolled ?? null;
  return (
    <>
      <h1>{securityText.title}</h1>
      <p className="muted">{securityText.intro}</p>
      {(required === "1" || profile?.totp_enforcement === "required") && !enrolled ? <RequiredPanel /> : null}
      <TotpNudge org={org} profile={profile} />
      {profile?.totp_pending && !enrolled ? <p className="muted">{securityText.status.pending}</p> : null}
      <TotpPanel
        initialEnrolled={enrolled}
        actions={{ enrol: enrolTotpAction, confirm: confirmTotpAction, disable: disableTotpAction }}
      />
    </>
  );
}
