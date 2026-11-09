import type { Metadata } from "next";
import { ClinicalSettingsForm, SignerProfileForm } from "@/components/clinical/ClinicalSettingsForm";
import { OrgLanguageForm } from "@/components/security/LanguageForms";
import { authedApi, currentRole } from "@/lib/auth/server";
import { loadOrNotFound } from "@/lib/loaders";
import { can } from "@/lib/permissions";
import { getText } from "@/lib/i18n/server";
import { saveClinicalSettingsAction, saveProfileAction } from "../../clinic/worklist/actions";
import { saveOrgLanguageAction } from "./actions";

export async function generateMetadata(): Promise<Metadata> {
  return { title: (await getText()).clinical.settings.title };
}

export default async function ClinicalSettingsPage({ params }: { params: Promise<{ org: string }> }) {
  const { org } = await params;
  const t = (await getText()).clinical.settings;
  const role = await currentRole();
  const api = await authedApi();
  const profile = await loadOrNotFound(() => api.getMyProfile());
  const canEdit = can(role, "org.update");
  // I18N-20: the organisation's default language, for owner and admin only (hidden for every other role).
  const orgRecord = canEdit ? await api.getOrg(org).catch(() => null) : null;
  return (
    <>
      <h1>{t.title}</h1>
      <p className="muted">{t.intro}</p>
      <h2>{t.profileTitle}</h2>
      <SignerProfileForm
        save={saveProfileAction.bind(null, org)}
        displayName={profile.display_name ?? ""}
        registration={profile.professional_registration ?? ""}
      />
      <h2>{t.title}</h2>
      <ClinicalSettingsForm save={saveClinicalSettingsAction.bind(null, org)} canEdit={canEdit} />
      {canEdit ? (
        <OrgLanguageForm save={saveOrgLanguageAction.bind(null, org)} current={orgRecord?.default_locale ?? null} />
      ) : null}
    </>
  );
}
