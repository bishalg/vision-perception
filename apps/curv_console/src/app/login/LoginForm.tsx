"use client";

import { useActionState } from "react";
import { en } from "@/messages/en";
import type { LoginState } from "./actions";

type Action = (prev: LoginState, formData: FormData) => Promise<LoginState>;

export function LoginForm({ action, next, mockHint }: { action: Action; next?: string; mockHint?: boolean }) {
  const [state, formAction, pending] = useActionState(action, { error: null });
  const t = en.login;
  return (
    <form action={formAction} className="login-form">
      <h1>{t.title}</h1>
      {next ? <input type="hidden" name="next" value={next} /> : null}
      <label>
        {t.email}
        <input name="email" type="email" autoComplete="username" required />
      </label>
      <label>
        {t.password}
        <input name="password" type="password" autoComplete="current-password" required />
      </label>
      {state.error ? (
        <p role="alert" className="error">
          {state.error === "failed" ? t.failed : t.unavailable}
        </p>
      ) : null}
      <button type="submit" disabled={pending}>
        {t.submit}
      </button>
      {mockHint ? <p className="muted">{t.mockHint}</p> : null}
    </form>
  );
}
