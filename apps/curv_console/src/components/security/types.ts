/** Outcome of a TOTP server action: the problem slug and status only, never the submitted fields. */
export type TotpFailure = { ok: false; slug: string | null; status: number };

export type TotpResult<T = object> = ({ ok: true } & T) | TotpFailure;

/** Outcome of a language server action (I18N-20): nothing but success or the failure's slug and status. */
export type LanguageResult = { ok: true } | TotpFailure;
