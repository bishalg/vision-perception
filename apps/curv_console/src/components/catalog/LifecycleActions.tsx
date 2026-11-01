"use client";

import { useState, useTransition } from "react";
import type { ConfigDocumentStatus, ConfigTransition } from "@/lib/api/catalog";
import { catalogText } from "@/messages/catalog";
import s from "./catalog.module.css";
import { catalogFailureText, type CatalogResult } from "./types";

const t = catalogText.lifecycle;

/** The steps the backend allows from each state (`config_docs::lifecycle`); it still checks state and four-eyes. */
export function stepsFor(status: ConfigDocumentStatus): ConfigTransition[] {
  switch (status) {
    case "draft":
      return ["approve", "retire"];
    case "candidate":
      return ["publish", "retire"];
    case "published":
      return ["retire"];
    default:
      return [];
  }
}

/** Approve / publish / retire buttons for a superadmin. Results and problem+json reasons are announced inline. */
export function LifecycleActions({
  status,
  run,
}: {
  status: ConfigDocumentStatus;
  run: (step: ConfigTransition) => Promise<CatalogResult<{ status: string }>>;
}) {
  const [pending, start] = useTransition();
  const [message, setMessage] = useState<{ ok: boolean; text: string } | null>(null);
  const [confirming, setConfirming] = useState(false);
  const steps = stepsFor(status);

  const go = (step: ConfigTransition) =>
    start(async () => {
      setConfirming(false);
      const r = await run(step);
      setMessage(r.ok ? { ok: true, text: t.done[step] } : { ok: false, text: catalogFailureText(r) });
    });

  return (
    <section className="panel stack" aria-label={t.title}>
      <h2 style={{ margin: 0 }}>{t.title}</h2>
      <p className="muted" style={{ margin: 0 }}>{t.hint[status]}</p>
      {steps.length ? (
        <div className={s.row}>
          {steps.includes("approve") ? (
            <button type="button" disabled={pending} onClick={() => go("approve")}>{t.approve}</button>
          ) : null}
          {steps.includes("publish") ? (
            <button type="button" disabled={pending} onClick={() => go("publish")}>{t.publish}</button>
          ) : null}
          {steps.includes("retire") && !confirming ? (
            <button type="button" className="danger" disabled={pending} onClick={() => setConfirming(true)}>{t.retire}</button>
          ) : null}
          {confirming ? (
            <span className="confirm" role="group" aria-label={t.confirmRetire}>
              <span>{t.confirmRetire}</span>
              <button type="button" className="danger" disabled={pending} onClick={() => go("retire")}>{t.confirm}</button>
              <button type="button" className="secondary" onClick={() => setConfirming(false)}>{t.cancel}</button>
            </span>
          ) : null}
        </div>
      ) : null}
      <p role="status" className={message ? (message.ok ? s.ok : s.alert) : undefined}>{message?.text ?? ""}</p>
    </section>
  );
}
