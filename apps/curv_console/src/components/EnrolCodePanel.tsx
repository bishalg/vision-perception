"use client";

import { useActionState } from "react";
import { formatDateTime } from "@/lib/format";
import { en } from "@/messages/en";
import { QrCode } from "./QrCode";

export type EnrolResult =
  | { ok: true; code: string; max_uses: number; expires_at: string }
  | { ok: false; error: string };

type State = EnrolResult | null;

const t = en.devices.enrol;

export function EnrolCodePanel({
  create,
  canCreate,
}: {
  create: (formData: FormData) => Promise<EnrolResult>;
  canCreate: boolean;
}) {
  const [state, formAction, pending] = useActionState<State, FormData>((_prev, fd) => create(fd), null);
  return (
    <section className="panel enrol" aria-label={t.title}>
      <h2>{t.title}</h2>
      {!canCreate ? (
        <p className="muted">{t.denied}</p>
      ) : (
        <>
          <p className="muted">{t.intro}</p>
          <form action={formAction} className="enrol-form">
            <label>
              {t.label}
              <input name="label" maxLength={200} autoComplete="off" />
            </label>
            <label>
              {t.maxUses}
              <input name="max_uses" type="number" min={1} max={1000} defaultValue={1} />
            </label>
            <label>
              {t.expiresDays}
              <input name="expires_in_days" type="number" min={1} max={90} defaultValue={7} />
            </label>
            <button type="submit" disabled={pending}>{t.generate}</button>
          </form>
          {state && !state.ok ? (
            <p role="alert" className="error">
              {t.failed} {state.error}
            </p>
          ) : null}
          {state?.ok ? (
            <div className="enrol-result">
              <QrCode value={state.code} label={t.qr(state.code)} />
              <div>
                <p className="kpi-label">{t.code}</p>
                <p className="enrol-code" data-testid="enrol-code">{state.code}</p>
                <p>{t.expires(formatDateTime(state.expires_at), state.max_uses)}</p>
                <p className="muted">{t.once}</p>
              </div>
            </div>
          ) : null}
        </>
      )}
    </section>
  );
}
