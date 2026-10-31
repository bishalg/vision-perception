import type { Metadata } from "next";
import { ClinicalSettingsForm, SignerProfileForm } from "@/components/clinical/ClinicalSettingsForm";
import { authedApi, currentRole } from "@/lib/auth/server";
import { loadOrNotFound } from "@/lib/loaders";
import { can } from "@/lib/permissions";
import { clinicalText } from "@/messages/clinical";
import { saveClinicalSettingsAction, saveProfileAction } from "../../worklist/actions";

const t = clinicalText.settings;

export const metadata: Metadata = { title: t.title };

export default async function ClinicalSettingsPage({ params }: { params: Promise<{ org: string }> }) {
  const { org } = await params;
  const role = await currentRole();
  const api = await authedApi();
  const profile = await loadOrNotFound(() => api.getMyProfile());
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
      <ClinicalSettingsForm save={saveClinicalSettingsAction.bind(null, org)} canEdit={can(role, "org.update")} />
    </>
  );
}
