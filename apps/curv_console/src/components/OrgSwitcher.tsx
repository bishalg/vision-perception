"use client";

import { en } from "@/messages/en";

const t = en.orgs;

/** Plain GET form to /switch-org (works without JS); the select submits on change. */
export function OrgSwitcher({
  current,
  orgs,
  allowAnyId,
}: {
  current: string;
  orgs: { id: string; name: string }[];
  allowAnyId: boolean;
}) {
  return (
    <form method="get" action="/switch-org" aria-label={t.formLabel} className="org-switcher">
      <select
          name="org"
          defaultValue={current}
          aria-label={t.label}
          onChange={(e) => e.currentTarget.form?.requestSubmit()}
        >
          {orgs.map((o) => (
            <option key={o.id} value={o.id}>
              {o.name}
            </option>
          ))}
      </select>
      {allowAnyId ? (
        <input name="org_id" placeholder={t.byId} aria-label={t.byId} size={14} autoComplete="off" />
      ) : null}
      <noscript>
        <button type="submit">{t.switch}</button>
      </noscript>
      {allowAnyId ? <button type="submit" className="link-button">{t.switch}</button> : null}
    </form>
  );
}
