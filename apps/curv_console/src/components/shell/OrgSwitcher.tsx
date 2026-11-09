"use client";

import { useText } from "@/lib/i18n/provider";
import s from "./shell.module.css";

/**
 * Plain GET form to /switch-org (works without JS); the select submits on change. WP-C.23: compact in the top bar
 * (names only). The superadmin's "Organisation id" field and "Switch" button sit behind a disclosure in the same form.
 */
export function OrgSwitcher({
  current,
  orgs,
  allowAnyId,
}: {
  current: string;
  orgs: { id: string; name: string }[];
  allowAnyId: boolean;
}) {
  const t = useText().app.orgs;
  return (
    <form method="get" action="/switch-org" aria-label={t.formLabel} className={s.orgSwitcher}>
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
      <noscript>
        <button type="submit">{t.switch}</button>
      </noscript>
      {allowAnyId ? (
        <details className={`${s.menu} ${s.orgById}`}>
          <summary>{t.byId}</summary>
          <div className={s.menuPanel}>
            <input name="org_id" placeholder={t.byId} aria-label={t.byId} size={14} autoComplete="off" />
            <button type="submit" className="link-button">
              {t.switch}
            </button>
          </div>
        </details>
      ) : null}
    </form>
  );
}
