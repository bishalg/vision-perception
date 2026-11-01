"use client";

import { useState, useTransition } from "react";
import s from "@/components/catalog/catalog.module.css";
import { catalogFailureText, type CatalogFailure, type CatalogResult } from "@/components/catalog/types";
import type { ConfigBundleSummary } from "@/lib/api/config";
import { configText } from "@/messages/config";

const t = configText.bundles;

/** `name=true` lines -> flags; the first bad line is returned as an error. */
export function parseFeatureFlags(text: string): { flags: Record<string, boolean> } | { error: string } {
  const flags: Record<string, boolean> = {};
  for (const raw of text.split("\n")) {
    const line = raw.trim();
    if (!line) continue;
    const m = /^([A-Za-z0-9_.-]{1,100})\s*=\s*(true|false)$/.exec(line);
    if (!m) return { error: t.flagsInvalid(line) };
    flags[m[1]!] = m[2] === "true";
  }
  return { flags };
}

function failureText(f: CatalogFailure): string {
  return (f.slug && configText.errors[f.slug]) || catalogFailureText(f);
}

/**
 * Compose and sign a bundle from the published documents (`POST /v1/config/bundles`). The approvals happened per
 * document (four-eyes); composing only signs what is published. Superadmins choose this org or the global default.
 */
export function BundleCompose({
  ready,
  superadmin,
  compose,
}: {
  /** Every kind has a published document. */
  ready: boolean;
  superadmin: boolean;
  compose: (input: { global: boolean; featureFlags: Record<string, boolean> }) => Promise<CatalogResult<{ bundle: ConfigBundleSummary }>>;
}) {
  const [global, setGlobal] = useState(false);
  const [flagsText, setFlagsText] = useState("");
  const [message, setMessage] = useState<{ ok: boolean; text: string } | null>(null);
  const [pending, start] = useTransition();
  const parsed = parseFeatureFlags(flagsText);

  function submit() {
    if ("error" in parsed) return;
    start(async () => {
      const r = await compose({ global: superadmin && global, featureFlags: parsed.flags });
      setMessage(r.ok ? { ok: true, text: t.composed(r.bundle.bundle_version) } : { ok: false, text: failureText(r) });
    });
  }

  return (
    <section className="panel stack" aria-label={t.compose}>
      {superadmin ? (
        <fieldset className={s.roles}>
          <legend>{t.scope}</legend>
          <label>
            <input type="radio" name="scope" checked={!global} onChange={() => setGlobal(false)} /> {t.scopeOrg}
          </label>
          <label>
            <input type="radio" name="scope" checked={global} onChange={() => setGlobal(true)} /> {t.scopeGlobal}
          </label>
        </fieldset>
      ) : null}
      <label>
        {t.flags}
        <textarea rows={3} value={flagsText} onChange={(ev) => setFlagsText(ev.currentTarget.value)} spellCheck={false}
          aria-invalid={"error" in parsed || undefined} className={"error" in parsed ? s.invalid : undefined} />
        {"error" in parsed ? <span className={s.fieldError}>{parsed.error}</span> : null}
      </label>
      {!ready ? <p className="muted">{t.cannotCompose}</p> : null}
      <div className={s.row}>
        <button type="button" disabled={pending || !ready || "error" in parsed} onClick={submit}>
          {pending ? t.composing : t.compose}
        </button>
      </div>
      <p role="status" className={message ? (message.ok ? s.ok : s.alert) : undefined}>{message?.text ?? ""}</p>
    </section>
  );
}
