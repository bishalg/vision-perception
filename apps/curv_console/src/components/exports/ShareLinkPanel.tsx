"use client";

import { useState, useTransition, type FormEvent } from "react";
import {
  SHARE_LINK_DAY_CHOICES,
  SHARE_LINK_DEFAULT_DAYS,
  SHARE_LINK_DEFAULT_DOWNLOADS,
  SHARE_LINK_DOWNLOAD_CHOICES,
  shareLinkState,
  type CreatedShareLink,
  type ShareLink,
} from "@/lib/api/share";
import { formatDateTime } from "@/lib/format";
import { shareLinksText } from "@/messages/exports";
import { failureText, type ActionFailure, type ActionResult } from "./types";

const t = shareLinksText;

export interface ShareLinkPanelProps {
  links: ShareLink[];
  /** The export is a ready case package (the backend refuses links otherwise). */
  ready: boolean;
  /** `sharelink.manage` for the signed-in role (hides controls only; the backend decides). */
  canManage: boolean;
  /** Request time, epoch ms, for the link states. */
  now: number;
  create: (form: FormData) => Promise<ActionResult<{ created: CreatedShareLink }>>;
  revoke: (linkId: string) => Promise<ActionResult>;
}

/** An absolute link for the recipient: the backend's `url` is relative when it does not know the console origin. */
function absolute(url: string): string {
  try {
    return new URL(url, window.location.origin).href;
  } catch {
    return url;
  }
}

function CopyButton({ value, label }: { value: string; label: string }) {
  const [state, setState] = useState<"idle" | "copied" | "failed">("idle");
  return (
    <>
      <button
        type="button"
        className="secondary"
        aria-label={`${t.copy} ${label}`}
        onClick={async () => {
          try {
            await navigator.clipboard.writeText(value);
            setState("copied");
          } catch {
            setState("failed");
          }
        }}
      >
        {state === "copied" ? t.copied : t.copy}
      </button>
      {state === "failed" ? <span className="error"> {t.copyFailed}</span> : null}
    </>
  );
}

/**
 * The passcode, shown once (SHR-8). It lives in this component's state only while the box is open; "close" drops it
 * and nothing else keeps a copy (no storage, no URL, no page data: the list never carries it).
 */
function CreatedBox({ created, onClose }: { created: CreatedShareLink; onClose: () => void }) {
  const url = absolute(created.link.url);
  return (
    <div role="dialog" aria-modal="false" aria-labelledby="share-created-h" className="panel stack" data-testid="share-created">
      <h3 id="share-created-h">{t.createdTitle}</h3>
      <p role="status"><strong>{t.once}</strong></p>
      <dl>
        <dt className="kpi-label">{t.url}</dt>
        <dd>
          <code data-testid="share-url">{url}</code> <CopyButton value={url} label={t.url} />
        </dd>
        <dt className="kpi-label">{t.passcode}</dt>
        <dd>
          <code data-testid="share-passcode">{created.passcode}</code> <CopyButton value={created.passcode} label={t.passcode} />
        </dd>
      </dl>
      <p>{t.otherChannel}</p>
      <div>
        <button type="button" onClick={onClose}>{t.done}</button>
      </div>
    </div>
  );
}

function RevokeButton({ link, revoke }: { link: ShareLink; revoke: ShareLinkPanelProps["revoke"] }) {
  const [confirming, setConfirming] = useState(false);
  const [pending, startTransition] = useTransition();
  const [failure, setFailure] = useState<ActionFailure | null>(null);
  if (!confirming) {
    return (
      <button type="button" className="danger" onClick={() => setConfirming(true)}>
        {t.revoke}
      </button>
    );
  }
  return (
    <div className="stack">
      <p role="alertdialog" aria-label={t.revoke}>{t.revokeConfirm}</p>
      <div className="filter-actions">
        <button
          type="button"
          className="danger"
          disabled={pending}
          onClick={() =>
            startTransition(async () => {
              const r = await revoke(link.id);
              if (r.ok) setConfirming(false);
              else setFailure(r);
            })
          }
        >
          {t.revokeYes}
        </button>
        <button type="button" onClick={() => setConfirming(false)} disabled={pending}>
          {t.cancel}
        </button>
      </div>
      {failure ? <p role="alert" className="error">{t.revokeFailed} {failureText(failure)}</p> : null}
    </div>
  );
}

/** Share links of one case-package export (SHR-8, SHR-10): create (passcode shown once), list, revoke. */
export function ShareLinkPanel({ links, ready, canManage, now, create, revoke }: ShareLinkPanelProps) {
  const [created, setCreated] = useState<CreatedShareLink | null>(null);
  const [failure, setFailure] = useState<ActionFailure | null>(null);
  const [pending, setPending] = useState(false);

  async function onCreate(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const data = new FormData(event.currentTarget);
    setPending(true);
    setFailure(null);
    setCreated(null);
    let r: ActionResult<{ created: CreatedShareLink }>;
    try {
      r = await create(data);
    } catch {
      r = { ok: false, slug: null, status: 0 };
    }
    setPending(false);
    if (r.ok) setCreated(r.created);
    else setFailure(r);
  }

  return (
    <section className="panel stack" aria-labelledby="share-links-h">
      <h2 id="share-links-h">{t.title}</h2>
      <p className="muted">{t.intro}</p>
      {!canManage ? <p className="muted">{t.denied}</p> : null}
      {canManage && !ready ? <p className="muted">{t.notReady}</p> : null}
      {canManage && ready && !created ? (
        <form onSubmit={onCreate} className="filter-actions">
          <label>
            {t.expires}
            <select name="expires_in_days" defaultValue={String(SHARE_LINK_DEFAULT_DAYS)}>
              {SHARE_LINK_DAY_CHOICES.map((d) => (
                <option key={d} value={d}>{t.days(d)}</option>
              ))}
            </select>
          </label>
          <label>
            {t.downloads}
            <select name="max_downloads" defaultValue={String(SHARE_LINK_DEFAULT_DOWNLOADS)}>
              {SHARE_LINK_DOWNLOAD_CHOICES.map((n) => (
                <option key={n} value={n}>{n}</option>
              ))}
            </select>
          </label>
          <button type="submit" disabled={pending}>{pending ? t.creating : t.create}</button>
        </form>
      ) : null}
      {failure ? <p role="alert" className="error">{t.failed} {failureText(failure)}</p> : null}
      {created ? <CreatedBox created={created} onClose={() => setCreated(null)} /> : null}
      {links.length === 0 ? (
        <p className="muted">{t.empty}</p>
      ) : (
        <table className="data" aria-label={t.title}>
          <thead>
            <tr>
              <th scope="col">{t.cols.created}</th>
              <th scope="col">{t.cols.state}</th>
              <th scope="col">{t.cols.downloads}</th>
              <th scope="col">{t.cols.opens}</th>
              <th scope="col">{t.cols.lastOpened}</th>
              <th scope="col">{t.cols.expires}</th>
              {canManage ? <th scope="col">{t.cols.actions}</th> : null}
            </tr>
          </thead>
          <tbody>
            {links.map((l) => {
              const state = shareLinkState(l, now);
              return (
                <tr key={l.id} data-link-id={l.id} data-state={state}>
                  <td>{formatDateTime(l.created_at)}</td>
                  <td>{t.states[state]}</td>
                  <td>{`${l.downloads ?? 0} / ${l.max_downloads}`}</td>
                  <td>{l.opens ?? 0}</td>
                  <td>{l.last_opened_at ? formatDateTime(l.last_opened_at) : "-"}</td>
                  <td>{formatDateTime(l.expires_at)}</td>
                  {canManage ? <td>{state === "revoked" ? null : <RevokeButton link={l} revoke={revoke} />}</td> : null}
                </tr>
              );
            })}
          </tbody>
        </table>
      )}
    </section>
  );
}
