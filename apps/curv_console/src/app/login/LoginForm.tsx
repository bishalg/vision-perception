"use client";

import { useActionState, useState } from "react";
import { messagesFor, type Messages } from "@/lib/i18n/messages";
import { useText } from "@/lib/i18n/provider";
import type { LoginState } from "./actions";

type Action = (prev: LoginState, formData: FormData) => Promise<LoginState>;

/** The sentence for a failed sign-in; `t` defaults to English for callers outside a component. */
export function loginErrorText(state: LoginState, t: Messages["app"]["login"] = messagesFor("en").app.login): string | null {
  switch (state.error) {
    case null:
      return null;
    case "failed":
      return t.failed;
    case "totp-invalid":
      return t.totpInvalid;
    case "rate-limited":
      return state.retryAfter != null ? t.rateLimitedAfter(state.retryAfter) : t.rateLimited;
    case "server-problem":
      return t.serverProblem;
    default:
      return t.unavailable;
  }
}

export function LoginForm({ action, next, mockHint }: { action: Action; next?: string; mockHint?: boolean }) {
  const [state, formAction, pending] = useActionState(action, { error: null, step: "password" });
  // Controlled, because React resets uncontrolled fields after an action; the password stays in memory
  // (posted by the server action, never put in the URL) across the TOTP step.
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [dismissed, setDismissed] = useState(false);
  const totp = state.step === "totp" && !dismissed;
  const t = useText().app.login;
  const error = loginErrorText(state, t);
  return (
    <form
      action={(fd) => {
        setDismissed(false);
        return formAction(fd);
      }}
      className="login-form"
    >
      <h1>{t.title}</h1>
      {next ? <input type="hidden" name="next" value={next} /> : null}
      {totp ? (
        <>
          <input type="hidden" name="email" value={email} />
          <input type="hidden" name="password" value={password} />
          <p className="muted">{t.totpPrompt}</p>
          <label>
            {t.totpCode}
            <input
              key={state.error ?? "fresh"}
              name="totp_code"
              type="text"
              inputMode="numeric"
              autoComplete="one-time-code"
              pattern="[0-9]{6}"
              maxLength={6}
              required
              autoFocus
            />
          </label>
        </>
      ) : (
        <>
          <label>
            {t.email}
            <input
              name="email"
              type="email"
              autoComplete="username"
              required
              value={email}
              onChange={(e) => setEmail(e.target.value)}
            />
          </label>
          <label>
            {t.password}
            <input
              name="password"
              type="password"
              autoComplete="current-password"
              required
              value={password}
              onChange={(e) => setPassword(e.target.value)}
            />
          </label>
        </>
      )}
      {error ? (
        <p role="alert" className="error">
          {error}
        </p>
      ) : null}
      <button type="submit" disabled={pending}>
        {t.submit}
      </button>
      {totp ? (
        <button
          type="button"
          className="link"
          onClick={() => {
            setPassword("");
            setDismissed(true);
          }}
        >
          {t.totpBack}
        </button>
      ) : null}
      {mockHint ? <p className="muted">{t.mockHint}</p> : null}
    </form>
  );
}
