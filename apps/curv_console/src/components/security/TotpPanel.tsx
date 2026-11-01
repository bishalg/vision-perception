"use client";

import { useActionState } from "react";
import { formatDateTime } from "@/lib/format";
import { securityErrorMessage, securityText } from "@/messages/security";
import { QrCode } from "../QrCode";
import type { TotpFailure, TotpResult } from "./types";

type Seed = { secret: string; otpauth_url: string; expires_at: string };

export interface TotpActions {
  enrol: (formData: FormData) => Promise<TotpResult<Seed>>;
  confirm: (formData: FormData) => Promise<TotpResult<{ totp_enrolled: boolean }>>;
  disable: (formData: FormData) => Promise<TotpResult<{ totp_enrolled: boolean }>>;
}

/**
 * One state for the whole flow. The pending seed lives only here, in memory, while it is on screen: it is dropped
 * as soon as the enrolment is confirmed, and nothing is written to storage. Forms are uncontrolled, so React clears
 * the password and code inputs after each action.
 */
type State = {
  seed: Seed | null;
  /** Last confirmed status from the backend; null until an action answers. */
  enrolled: boolean | null;
  error: { form: "enrol" | "confirm" | "disable"; failure: TotpFailure } | null;
};

const t = securityText;

/** `initialEnrolled`: `totp_enrolled` from `GET /users/me`, or null when unknown. */
export function TotpPanel({ actions, initialEnrolled = null }: { actions: TotpActions; initialEnrolled?: boolean | null }) {
  const initial: State = { seed: null, enrolled: initialEnrolled, error: null };
  const [state, dispatch, pending] = useActionState<State, FormData>(async (prev, fd) => {
    const intent = fd.get("intent");
    if (intent === "enrol") {
      const res = await actions.enrol(fd);
      if (!res.ok) return { ...prev, seed: null, error: { form: "enrol", failure: res } };
      return { seed: { secret: res.secret, otpauth_url: res.otpauth_url, expires_at: res.expires_at }, enrolled: null, error: null };
    }
    if (intent === "confirm") {
      const res = await actions.confirm(fd);
      // An expired or already-confirmed enrolment cannot be confirmed again: drop the seed.
      if (!res.ok) {
        const keep = res.slug !== "totp-enrolment-expired" && res.slug !== "totp-already-enrolled";
        return { ...prev, seed: keep ? prev.seed : null, error: { form: "confirm", failure: res } };
      }
      return { seed: null, enrolled: res.totp_enrolled, error: null };
    }
    if (intent === "disable") {
      const res = await actions.disable(fd);
      if (!res.ok) return { ...prev, error: { form: "disable", failure: res } };
      return { seed: null, enrolled: res.totp_enrolled, error: null };
    }
    return prev;
  }, initial);

  const errorFor = (form: "enrol" | "confirm" | "disable") =>
    state.error?.form === form ? (
      <p role="alert" className="error">
        {securityErrorMessage(state.error.failure.slug, state.error.failure.status)}
      </p>
    ) : null;

  return (
    <>
      {state.enrolled !== null ? (
        <p role="status" className="panel">
          {state.enrolled ? t.enrol.done : t.disable.done}
        </p>
      ) : null}

      <section className="panel enrol" aria-label={t.enrol.title}>
        <h2>{t.enrol.title}</h2>
        {state.seed ? (
          <>
            <div className="enrol-result">
              <QrCode value={state.seed.otpauth_url} label={t.enrol.qr} />
              <div>
                <p>{t.enrol.scan}</p>
                <p className="kpi-label">{t.enrol.secret}</p>
                <p className="enrol-code totp-secret" data-testid="totp-secret">
                  {state.seed.secret}
                </p>
                <p>{t.enrol.expires(formatDateTime(state.seed.expires_at))}</p>
                <p className="muted">{t.enrol.once}</p>
              </div>
            </div>
            <form key="confirm" action={dispatch} className="enrol-form">
              <input type="hidden" name="intent" value="confirm" />
              <label>
                {t.enrol.code}
                <input
                  name="totp_code"
                  inputMode="numeric"
                  autoComplete="one-time-code"
                  pattern="[0-9]{6}"
                  maxLength={6}
                  required
                />
              </label>
              <button type="submit" disabled={pending}>
                {t.enrol.confirm}
              </button>
            </form>
            {errorFor("confirm")}
          </>
        ) : (
          <>
            <p className="muted">{t.enrol.intro}</p>
            <form key="enrol" action={dispatch} className="enrol-form">
              <input type="hidden" name="intent" value="enrol" />
              <label>
                {t.enrol.password}
                <input name="password" type="password" autoComplete="current-password" required />
              </label>
              <button type="submit" disabled={pending}>
                {t.enrol.start}
              </button>
            </form>
            {errorFor("enrol")}
            {errorFor("confirm")}
          </>
        )}
      </section>

      <section className="panel enrol" aria-label={t.disable.title}>
        <h2>{t.disable.title}</h2>
        <p className="muted">{t.disable.intro}</p>
        <form action={dispatch} className="enrol-form">
          <input type="hidden" name="intent" value="disable" />
          <label>
            {t.disable.password}
            <input name="password" type="password" autoComplete="current-password" required />
          </label>
          <label>
            {t.disable.code}
            <input
              name="totp_code"
              inputMode="numeric"
              autoComplete="one-time-code"
              pattern="[0-9]{6}"
              maxLength={6}
              required
            />
          </label>
          <button type="submit" disabled={pending}>
            {t.disable.submit}
          </button>
        </form>
        {errorFor("disable")}
      </section>
    </>
  );
}
