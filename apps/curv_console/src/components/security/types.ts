/** Outcome of a TOTP server action: the problem slug and status only, never the submitted fields. */
export type TotpFailure = { ok: false; slug: string | null; status: number };

export type TotpResult<T = object> = ({ ok: true } & T) | TotpFailure;
