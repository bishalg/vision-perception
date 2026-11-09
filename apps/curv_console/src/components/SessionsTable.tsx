"use client";

import Link from "next/link";
import type { SessionSummary } from "@/lib/api";
import { formatDateTime, formatDuration, formatNumber, formatPercent } from "@/lib/format";
import { useText } from "@/lib/i18n/provider";
import { enumLabel } from "@/messages/domain";
import { FlagBadge } from "./FlagBadge";
import { ScoreValue } from "./ScoringStatus";
import { TrackingBadge } from "./TrackingQuality";
import { pathsFor } from "@/lib/paths";

export function SessionsTable({ orgId, sessions }: { orgId: string; sessions: SessionSummary[] }) {
  const t = useText();
  const c = t.app.sessions.cols;
  if (sessions.length === 0) return <p className="muted">{t.app.sessions.empty}</p>;
  return (
    <div className="table-wrap">
      <table className="data responsive">
        <thead>
          <tr>
            <th scope="col">{c.started}</th>
            <th scope="col">{c.participant}</th>
            <th scope="col">{c.flag}</th>
            <th scope="col" className="num">{c.composite}</th>
            <th scope="col" className="num">{c.confidence}</th>
            <th scope="col">{c.quality}</th>
            <th scope="col">{c.tracking}</th>
            <th scope="col" className="num">{c.duration}</th>
            <th scope="col">{c.bundle}</th>
            <th scope="col">{c.reviewed}</th>
          </tr>
        </thead>
        <tbody>
          {sessions.map((s) => (
            <tr key={s.id}>
              <td data-label={c.started}>
                <Link href={pathsFor(orgId).clinic.session(s.id)}>
                  {formatDateTime(s.started_at)}
                </Link>
              </td>
              <td data-label={c.participant}>{s.participant_token}</td>
              <td data-label={c.flag}><FlagBadge flag={s.attention_flag} /></td>
              <td data-label={c.composite} className="num">
                <ScoreValue session={s} value={formatNumber(s.composite_score, 1, t.locale)} />
              </td>
              <td data-label={c.confidence} className="num">{formatPercent(s.composite_confidence, t.locale)}</td>
              <td data-label={c.quality}>{enumLabel(t.locale, "quality_verdict", s.quality_verdict)}</td>
              <td data-label={c.tracking}>
                <TrackingBadge verdict={s.tracking_verdict} />
                {s.tracking_accuracy_deg != null ? ` ${t.app.detail.tracking.deg(s.tracking_accuracy_deg)}` : ""}
              </td>
              <td data-label={c.duration} className="num">{formatDuration(s.duration_sec)}</td>
              <td data-label={c.bundle}>{s.bundle_version ?? "-"}</td>
              <td data-label={c.reviewed}>{s.reviewed ? t.app.common.yes : t.app.common.no}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
