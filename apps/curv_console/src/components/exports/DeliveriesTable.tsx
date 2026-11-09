"use client";

import Link from "next/link";
import type { Delivery, DeliveryStatus } from "@/lib/api/exports";
import { formatDateTime } from "@/lib/format";
import { useText } from "@/lib/i18n/provider";
import { orgPaths } from "@/lib/paths";

const BADGE: Record<DeliveryStatus, string> = {
  pending: "badge-yellow",
  sending: "badge-yellow",
  delivered: "badge-green",
  failed: "badge-red",
};

/**
 * Delivery log rows. With `orgPath` (`/{org}`, see `orgBase`), `linkExports`/`linkDestinations` link the other side
 * of the relation to its page (`data.export(id)` / `admin.integration(id)`); plain values (not functions), so a
 * server page can pass them to this client component.
 */
export function DeliveriesTable({
  items,
  orgPath,
  linkExports = false,
  linkDestinations = false,
}: {
  items: Delivery[];
  orgPath?: string;
  linkExports?: boolean;
  linkDestinations?: boolean;
}) {
  const t = useText().exports.deliveries;
  const c = t.cols;
  const paths = orgPath ? orgPaths(orgPath) : undefined;
  const exportHref = paths && linkExports ? paths.data.export : undefined;
  const destinationHref = paths && linkDestinations ? paths.admin.integration : undefined;
  if (items.length === 0) return <p className="muted">{t.empty}</p>;
  return (
    <div className="table-wrap">
      <table className="data responsive">
        <thead>
          <tr>
            <th scope="col">{c.created}</th>
            <th scope="col">{c.destination}</th>
            {exportHref ? <th scope="col">{c.export}</th> : null}
            <th scope="col">{c.event}</th>
            <th scope="col">{c.status}</th>
            <th scope="col">{c.attempts}</th>
            <th scope="col">{c.error}</th>
            <th scope="col">{c.ref}</th>
            <th scope="col">{c.delivered}</th>
          </tr>
        </thead>
        <tbody>
          {items.map((d) => (
            <tr key={d.id}>
              <td data-label={c.created}>{formatDateTime(d.created_at)}</td>
              <td data-label={c.destination}>
                {destinationHref ? <Link href={destinationHref(d.destination_id)}>{d.destination_name}</Link> : d.destination_name}
              </td>
              {exportHref ? (
                <td data-label={c.export}>
                  <Link href={exportHref(d.export_id)}>{d.export_id.slice(0, 8)}</Link>
                </td>
              ) : null}
              <td data-label={c.event}>{t.events[d.event]}</td>
              <td data-label={c.status}><span className={`badge ${BADGE[d.status]}`}>{t.statuses[d.status]}</span></td>
              <td data-label={c.attempts}>{d.attempts}</td>
              <td data-label={c.error}>{d.last_error_class ?? "-"}</td>
              <td data-label={c.ref}>{d.remote_ref ?? "-"}</td>
              <td data-label={c.delivered}>{d.delivered_at ? formatDateTime(d.delivered_at) : "-"}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
