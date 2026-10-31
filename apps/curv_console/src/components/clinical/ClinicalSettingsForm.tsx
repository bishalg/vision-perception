"use client";

import { useActionState } from "react";
import { COSIGN_POLICIES, PAPERS, PATIENT_REF_MODES, WORKLIST_SCOPES } from "@/lib/api/clinical";
import { clinicalErrorMessage, clinicalText } from "@/messages/clinical";

const t = clinicalText.settings;

export type SettingsActionResult =
  | { ok: true }
  | { ok: false; slug: string | null; status: number; params?: string };

type State = SettingsActionResult | null;

function Outcome({ state, saved, failed }: { state: State; saved: string; failed: string }) {
  if (!state) return null;
  if (state.ok) return <p role="status">{saved}</p>;
  return (
    <p role="alert" className="error">
      {failed} {state.params ?? clinicalErrorMessage(state.slug, state.status)}
    </p>
  );
}

/** Org-level clinical settings. Empty selections mean "leave unchanged": the API does not return current settings. */
export function ClinicalSettingsForm({
  save,
  canEdit,
}: {
  save: (formData: FormData) => Promise<SettingsActionResult>;
  canEdit: boolean;
}) {
  const [state, action, pending] = useActionState<State, FormData>((_prev, fd) => save(fd), null);
  if (!canEdit) return <p className="muted">{t.readOnly}</p>;
  return (
    <form action={action} className="stack panel" aria-label={t.title}>
      <p className="muted">{t.currentUnknown}</p>
      <label>
        {t.worklistScope}
        <select name="worklist_scope" defaultValue="">
          <option value="">{t.unchanged}</option>
          {WORKLIST_SCOPES.map((v) => (
            <option key={v} value={v}>{t.scopes[v]}</option>
          ))}
        </select>
      </label>
      <label>
        {t.cosignPolicy}
        <select name="cosign_policy" defaultValue="">
          <option value="">{t.unchanged}</option>
          {COSIGN_POLICIES.map((v) => (
            <option key={v} value={v}>{t.cosignPolicies[v]}</option>
          ))}
        </select>
      </label>
      <label>
        {t.patientRefMode}
        <select name="patient_ref_mode" defaultValue="">
          <option value="">{t.unchanged}</option>
          {PATIENT_REF_MODES.map((v) => (
            <option key={v} value={v}>{t.patientRefModes[v]}</option>
          ))}
        </select>
      </label>
      <label>
        {t.timezone}
        <input name="timezone" maxLength={64} autoComplete="off" placeholder="UTC" />
      </label>
      <label>
        {t.paper}
        <select name="paper" defaultValue="">
          <option value="">{t.unchanged}</option>
          {PAPERS.map((v) => (
            <option key={v} value={v}>{t.papers[v]}</option>
          ))}
        </select>
      </label>
      <div>
        <button type="submit" disabled={pending}>{t.save}</button>
      </div>
      <Outcome state={state} saved={t.saved} failed={t.failed} />
    </form>
  );
}

/** Signer profile (NOTE-6): the user edits their own display name and registration. */
export function SignerProfileForm({
  save,
  displayName,
  registration,
}: {
  save: (formData: FormData) => Promise<SettingsActionResult>;
  displayName: string;
  registration: string;
}) {
  const [state, action, pending] = useActionState<State, FormData>((_prev, fd) => save(fd), null);
  return (
    <form action={action} className="stack panel" aria-label={t.profileTitle}>
      <p className="muted">{t.profileIntro}</p>
      <label>
        {t.displayName}
        <input name="display_name" maxLength={120} defaultValue={displayName} autoComplete="name" />
      </label>
      <label>
        {t.registration}
        <input name="professional_registration" maxLength={60} defaultValue={registration} autoComplete="off" />
      </label>
      <div>
        <button type="submit" disabled={pending}>{t.saveProfile}</button>
      </div>
      <Outcome state={state} saved={t.profileSaved} failed={t.profileFailed} />
    </form>
  );
}
