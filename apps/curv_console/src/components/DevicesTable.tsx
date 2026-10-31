"use client";

import { useState, useTransition } from "react";
import type { Device } from "@/lib/api";
import { formatDateTime } from "@/lib/format";
import { en } from "@/messages/en";

const t = en.devices;
const c = t.cols;

export type RevokeFn = (deviceId: string) => Promise<{ ok: boolean }>;

function RevokeButton({ device, revoke }: { device: Device; revoke: RevokeFn }) {
  const [confirming, setConfirming] = useState(false);
  const [failed, setFailed] = useState(false);
  const [pending, startTransition] = useTransition();
  if (!confirming) {
    return (
      <button type="button" className="danger" onClick={() => setConfirming(true)}>
        {t.revoke}
      </button>
    );
  }
  return (
    <span className="confirm">
      <button
        type="button"
        className="danger"
        disabled={pending}
        onClick={() =>
          startTransition(async () => {
            const res = await revoke(device.id);
            setFailed(!res.ok);
            if (res.ok) setConfirming(false);
          })
        }
      >
        {t.confirmRevoke}
      </button>
      <button type="button" className="link-button" onClick={() => setConfirming(false)}>
        {t.cancel}
      </button>
      {failed ? <span role="alert" className="error">{t.revokeFailed}</span> : null}
    </span>
  );
}

export function DevicesTable({ devices, canRevoke, revoke }: { devices: Device[]; canRevoke: boolean; revoke: RevokeFn }) {
  if (devices.length === 0) return <p className="muted">{t.empty}</p>;
  return (
    <div className="table-wrap">
      <table className="data responsive">
        <thead>
          <tr>
            <th scope="col">{c.label}</th>
            <th scope="col">{c.platform}</th>
            <th scope="col">{c.appVersion}</th>
            <th scope="col">{c.bundle}</th>
            <th scope="col">{c.enrolled}</th>
            <th scope="col">{c.lastSeen}</th>
            <th scope="col">{c.status}</th>
            {canRevoke ? <th scope="col">{c.actions}</th> : null}
          </tr>
        </thead>
        <tbody>
          {devices.map((d) => (
            <tr key={d.id}>
              <td data-label={c.label}>{d.label ?? d.id.slice(0, 8)}</td>
              <td data-label={c.platform}>{d.platform}</td>
              <td data-label={c.appVersion}>{d.app_version}</td>
              <td data-label={c.bundle}>{d.config_bundle_version ?? "-"}</td>
              <td data-label={c.enrolled}>{formatDateTime(d.enrolled_at)}</td>
              <td data-label={c.lastSeen}>{d.last_seen_at ? formatDateTime(d.last_seen_at) : "-"}</td>
              <td data-label={c.status}>
                <span className={`badge ${d.revoked_at ? "badge-unreliable" : "badge-green"}`}>
                  {d.revoked_at ? t.revoked : t.active}
                </span>
              </td>
              {canRevoke ? (
                <td data-label={c.actions}>{d.revoked_at ? null : <RevokeButton device={d} revoke={revoke} />}</td>
              ) : null}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
