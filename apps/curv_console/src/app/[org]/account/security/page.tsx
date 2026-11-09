import type { Metadata } from "next";
import { LanguageForm } from "@/components/security/LanguageForms";
import { TotpNudge } from "@/components/security/TotpNudge";
import { TotpPanel } from "@/components/security/TotpPanel";
import { RequiredPanel } from "@/components/security/RequiredPanel";
import { authedApi } from "@/lib/auth/server";
import { getText } from "@/lib/i18n/server";
import { confirmTotpAction, disableTotpAction, enrolTotpAction, saveLanguageAction } from "./actions";

export async function generateMetadata(): Promise<Metadata> {
  return { title: (await getText()).security.security.title };
}

/**
 * WP-C.15: the signed-in user's own second factor (any role). The current state comes from `GET /users/me`
 * (`totp_enrolled`, `totp_pending`); when that read fails the page still offers both setup and turn-off, and the
 * backend answers `totp-already-enrolled` / `totp-not-enrolled` when a step does not apply. WP-C.17: the layout sends
 * users past their deadline here with `?required=1`, which shows the required-state panel until they enrol.
 * I18N-20: the page also holds the user's own language setting (every role).
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
  const t = (await getText()).security.security;
  const api = await authedApi();
  const profile = await api.getMyProfile().catch(() => null);
  const orgRecord = await api.getOrg(org).catch(() => null);
  const enrolled = profile?.totp_enrolled ?? null;
  return (
    <>
      <h1>{t.title}</h1>
      <p className="muted">{t.intro}</p>
      {(required === "1" || profile?.totp_enforcement === "required") && !enrolled ? <RequiredPanel /> : null}
      <TotpNudge org={org} profile={profile} />
      {profile?.totp_pending && !enrolled ? <p className="muted">{t.status.pending}</p> : null}
      <LanguageForm
        save={saveLanguageAction}
        current={profile?.locale ?? null}
        orgLocale={orgRecord?.default_locale ?? null}
      />
      <TotpPanel
        initialEnrolled={enrolled}
        actions={{ enrol: enrolTotpAction, confirm: confirmTotpAction, disable: disableTotpAction }}
      />
    </>
  );
}
