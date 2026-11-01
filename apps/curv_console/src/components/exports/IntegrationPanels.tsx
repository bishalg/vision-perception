"use client";

import { useRouter } from "next/navigation";
import { useActionState, useState } from "react";
import type { Integration, IntegrationTestResult, SshPublicKey } from "@/lib/api/exports";
import { formatDateTime } from "@/lib/format";
import { integrationsText } from "@/messages/exports";
import { failureText, type ActionResult } from "./types";

/**
 * Secret panel (webhook). The secret is write-only: a pasted one is sent and dropped; a generated one is returned
 * by the server action once and shown in this component's state only (never persisted, gone on reload).
 */
export function SecretPanel({
  integration,
  put,
  canManage,
}: {
  integration: Pick<Integration, "has_secret" | "secret_fingerprint">;
  put: (formData: FormData) => Promise<ActionResult<{ generated?: string }>>;
  canManage: boolean;
}) {
  const t = integrationsText.secret;
  const [state, action, pending] = useActionState<ActionResult<{ generated?: string }> | null, FormData>(
    (_prev, fd) => put(fd),
    null,
  );
  return (
    <section className="panel stack" aria-label={t.title}>
      <h2>{t.title}</h2>
      <p className="muted">{t.intro}</p>
      <p>{integration.has_secret ? t.present(integration.secret_fingerprint) : t.absent}</p>
      {canManage ? (
        <form action={action} className="stack">
          <label>
            {t.custom}
            <input name="secret" type="password" autoComplete="off" spellCheck={false} />
          </label>
          <div>
            <button type="submit" disabled={pending}>{integration.has_secret ? t.rotate : t.set}</button>
          </div>
        </form>
      ) : null}
      {state?.ok ? <p role="status">{t.saved}</p> : null}
      {state?.ok && state.generated ? (
        <div>
          <p className="kpi-label">{t.generatedTitle}</p>
          <p className="enrol-code" data-testid="generated-secret">{state.generated}</p>
          <p className="muted">{t.generatedOnce}</p>
        </div>
      ) : null}
      {state && !state.ok ? <p role="alert" className="error">{t.failed} {failureText(state)}</p> : null}
    </section>
  );
}

/** SSH key panel (sftp): shows the stored public key; generating or rotating returns only the public half. */
export function SshKeyPanel({
  integration,
  generate,
  canManage,
}: {
  integration: Pick<Integration, "ssh_public_key">;
  generate: () => Promise<ActionResult<{ key: SshPublicKey }>>;
  canManage: boolean;
}) {
  const t = integrationsText.ssh;
  const [state, action, pending] = useActionState<ActionResult<{ key: SshPublicKey }> | null, FormData>(
    () => generate(),
    null,
  );
  const [confirming, setConfirming] = useState(false);
  const key = state?.ok ? state.key.public_key : integration.ssh_public_key;
  const rotating = Boolean(key);
  return (
    <section className="panel stack" aria-label={t.title}>
      <h2>{t.title}</h2>
      <p className="muted">{t.intro}</p>
      {key ? (
        <div>
          <p className="kpi-label">{t.publicKey}</p>
          <pre className="enrol-code" data-testid="ssh-public-key" style={{ whiteSpace: "pre-wrap", wordBreak: "break-all" }}>{key}</pre>
          {state?.ok ? <p>{t.fingerprint}: <code>{state.key.fingerprint}</code></p> : null}
        </div>
      ) : (
        <p>{t.none}</p>
      )}
      {canManage ? (
        <form action={action} className="stack">
          {rotating ? <p className="muted">{t.rotateWarn}</p> : null}
          {rotating && !confirming ? (
            <div>
              <button type="button" className="danger" onClick={() => setConfirming(true)}>{t.rotate}</button>
            </div>
          ) : (
            <div>
              <button type="submit" disabled={pending} className={rotating ? "danger" : undefined}>
                {rotating ? t.rotate : t.generate}
              </button>
            </div>
          )}
        </form>
      ) : null}
      {state?.ok ? <p role="status">{t.generated}</p> : null}
      {state && !state.ok ? <p role="alert" className="error">{t.failed} {failureText(state)}</p> : null}
    </section>
  );
}

/** Connection test: the backend sends a synthetic record; only the outcome and an error class come back. */
export function TestPanel({
  test,
  canManage,
}: {
  test: () => Promise<ActionResult<{ result: IntegrationTestResult }>>;
  canManage: boolean;
}) {
  const t = integrationsText.test;
  const [state, action, pending] = useActionState<ActionResult<{ result: IntegrationTestResult }> | null, FormData>(
    () => test(),
    null,
  );
  return (
    <section className="panel stack" aria-label={t.title}>
      <h2>{t.title}</h2>
      <p className="muted">{t.intro}</p>
      {canManage ? (
        <form action={action}>
          <button type="submit" disabled={pending}>{t.run}</button>
        </form>
      ) : null}
      {state?.ok ? (
        <div role="status">
          <p>
            <span className={`badge ${state.result.ok ? "badge-green" : "badge-red"}`}>
              {state.result.ok ? t.ok : t.failed}
            </span>
          </p>
          <p className="muted">
            {t.at}: {formatDateTime(state.result.tested_at)}
            {state.result.error_class ? ` | ${t.errorClass}: ${state.result.error_class}` : ""}
            {state.result.remote_ref ? ` | ${t.ref}: ${state.result.remote_ref}` : ""}
          </p>
        </div>
      ) : null}
      {state && !state.ok ? <p role="alert" className="error">{t.unavailable} {failureText(state)}</p> : null}
    </section>
  );
}

/**
 * Delete (archive) the destination. The confirmation is in the page, not a browser dialog. A 409 means deliveries
 * to it are still in flight; on success the user is taken back to the list.
 */
export function DeleteIntegrationPanel({
  name,
  remove,
  listHref,
}: {
  name: string;
  remove: () => Promise<ActionResult>;
  listHref: string;
}) {
  const t = integrationsText.delete;
  const router = useRouter();
  const [confirming, setConfirming] = useState(false);
  const [state, action, pending] = useActionState<ActionResult | null, FormData>(async () => {
    const result = await remove();
    if (result.ok) router.push(listHref);
    return result;
  }, null);
  return (
    <section className="panel stack" aria-label={t.title}>
      <h2>{t.title}</h2>
      <p className="muted">{t.intro}</p>
      {!confirming ? (
        <div>
          <button type="button" className="danger" onClick={() => setConfirming(true)}>{t.open}</button>
        </div>
      ) : (
        <form action={action} className="stack">
          <p role="alertdialog" aria-label={t.title}>{t.confirm(name)}</p>
          <div className="filter-actions">
            <button type="submit" className="danger" disabled={pending}>{t.confirmButton}</button>
            <button type="button" disabled={pending} onClick={() => setConfirming(false)}>{t.cancel}</button>
          </div>
        </form>
      )}
      {state?.ok ? <p role="status">{t.deleted}</p> : null}
      {state && !state.ok ? (
        <p role="alert" className="error">
          {state.status === 409 ? t.inFlight : `${t.failed} ${failureText(state)}`}
        </p>
      ) : null}
    </section>
  );
}
