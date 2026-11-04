"use client";

import { useActionState } from "react";
import { useText } from "@/lib/i18n/provider";
import type { Locale } from "@/lib/i18n/types";
import { securityErrorMessage } from "@/messages/security";
import type { LanguageResult } from "./types";

type State = LanguageResult | null;

/**
 * I18N-20: the signed-in user's own language. The empty option is "follow the organisation" (`locale: null`). The
 * server action saves it and revalidates the layout, so the page re-renders in the chosen language.
 */
export function LanguageForm({
  save,
  current,
  orgLocale,
}: {
  save: (formData: FormData) => Promise<LanguageResult>;
  /** The user's own setting; `null` follows the organisation. */
  current: Locale | null;
  /** The organisation's default language (`null` = not set, English). */
  orgLocale: Locale | null;
}) {
  const t = useText().security.security;
  const l = t.language;
  const [state, action, pending] = useActionState<State, FormData>((_prev, fd) => save(fd), null);
  const orgName = orgLocale === "ne" ? l.nepali : l.english;
  return (
    <section className="panel stack" aria-label={l.title}>
      <h2>{l.title}</h2>
      <p className="muted">{l.intro}</p>
      <form action={action} className="stack">
        <label>
          {l.label}
          <select name="locale" key={current ?? ""} defaultValue={current ?? ""}>
            <option value="">{l.follow(orgName)}</option>
            <option value="en">{l.english}</option>
            <option value="ne">{l.nepali}</option>
          </select>
        </label>
        <div>
          <button type="submit" disabled={pending}>{l.save}</button>
        </div>
      </form>
      {state?.ok ? <p role="status">{l.saved}</p> : null}
      {state && !state.ok ? (
        <p role="alert" className="error">{l.failed} {securityErrorMessage(state.slug, state.status, t.errors)}</p>
      ) : null}
    </section>
  );
}

/** I18N-20: the organisation's default language (owner and admin). Rendered only for roles that may update the org. */
export function OrgLanguageForm({
  save,
  current,
}: {
  save: (formData: FormData) => Promise<LanguageResult>;
  current: Locale | null;
}) {
  const t = useText().security.security;
  const l = t.orgLanguage;
  const [state, action, pending] = useActionState<State, FormData>((_prev, fd) => save(fd), null);
  return (
    <section className="panel stack" aria-label={l.title}>
      <h2>{l.title}</h2>
      <p className="muted">{l.intro}</p>
      <form action={action} className="stack">
        <label>
          {l.label}
          <select name="default_locale" key={current ?? "en"} defaultValue={current ?? "en"}>
            <option value="en">{l.english}</option>
            <option value="ne">{l.nepali}</option>
          </select>
        </label>
        <div>
          <button type="submit" disabled={pending}>{l.save}</button>
        </div>
      </form>
      {state?.ok ? <p role="status">{l.saved}</p> : null}
      {state && !state.ok ? (
        <p role="alert" className="error">{l.failed} {securityErrorMessage(state.slug, state.status, t.errors)}</p>
      ) : null}
    </section>
  );
}
