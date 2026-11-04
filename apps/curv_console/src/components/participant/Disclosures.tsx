"use client";

import Link from "next/link";
import type { Disclosure } from "@/lib/api/participant";
import { formatDateTime } from "@/lib/format";
import { useText } from "@/lib/i18n/provider";
import { labelOr } from "@/messages/participant";

export function Disclosures({
  items,
  basePath,
  nextCursor,
  hasCursor,
}: {
  items: Disclosure[];
  basePath: string;
  nextCursor: string | null | undefined;
  hasCursor: boolean;
}) {
  const text = useText();
  const t = text.participant.disclosures;
  const c = t.cols;
  const { locale } = text;
  return (
    <section aria-labelledby="disc-h">
      <h2 id="disc-h">{t.title}</h2>
      <p className="muted">{t.intro}</p>
      {items.length === 0 ? (
        <p className="muted">{t.empty}</p>
      ) : (
        <div className="table-wrap">
          <table className="data responsive">
            <thead>
              <tr>
                <th scope="col">{c.when}</th>
                <th scope="col">{c.what}</th>
                <th scope="col">{c.mode}</th>
                <th scope="col">{c.purpose}</th>
                <th scope="col">{c.status}</th>
                <th scope="col">{c.sessions}</th>
                <th scope="col">{c.deliveries}</th>
              </tr>
            </thead>
            <tbody>
              {items.map((d) => (
                <tr key={d.export_id}>
                  <td data-label={c.when}>{formatDateTime(d.created_at)}</td>
                  <td data-label={c.what}>
                    {labelOr(locale, "export_format", d.format)}
                    {d.requested_by ? <span className="muted"> {t.byline(d.requested_by)}</span> : null}
                  </td>
                  <td data-label={c.mode}>{labelOr(locale, "export_mode", d.mode)}</td>
                  <td data-label={c.purpose}>{labelOr(locale, "export_purpose", d.purpose)}</td>
                  <td data-label={c.status}>{labelOr(locale, "export_status", d.status)}</td>
                  <td data-label={c.sessions}>{d.session_ids.length}</td>
                  <td data-label={c.deliveries}>
                    {d.deliveries.length === 0 ? (
                      t.noDeliveries
                    ) : (
                      <ul className="plain">
                        {d.deliveries.map((x) => (
                          <li key={x.destination_id}>
                            {t.delivery(
                              x.destination_name,
                              t.deliveryStatuses[x.status] ?? x.status,
                              x.delivered_at ? formatDateTime(x.delivered_at) : null,
                            )}
                          </li>
                        ))}
                      </ul>
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
      <nav className="pagination" aria-label={t.pages}>
        {hasCursor ? <Link href={`${basePath}?tab=disclosures`}>{t.newest}</Link> : null}
        {nextCursor ? (
          <Link rel="next" href={`${basePath}?tab=disclosures&cursor=${encodeURIComponent(nextCursor)}`}>{t.older}</Link>
        ) : null}
      </nav>
    </section>
  );
}
