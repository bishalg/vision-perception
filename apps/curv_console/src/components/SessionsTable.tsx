import Link from "next/link";
import { en } from "@/messages/en";
import type { SessionSummary } from "@/lib/api";
import { formatDateTime, formatDuration, formatNumber, formatPercent } from "@/lib/format";
import { FlagBadge } from "./FlagBadge";

const c = en.sessions.cols;

export function SessionsTable({ orgId, sessions }: { orgId: string; sessions: SessionSummary[] }) {
  if (sessions.length === 0) return <p className="muted">{en.sessions.empty}</p>;
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
            <th scope="col" className="num">{c.duration}</th>
            <th scope="col">{c.bundle}</th>
            <th scope="col">{c.reviewed}</th>
          </tr>
        </thead>
        <tbody>
          {sessions.map((s) => (
            <tr key={s.id}>
              <td data-label={c.started}>
                <Link href={`/${encodeURIComponent(orgId)}/sessions/${encodeURIComponent(s.id)}`}>
                  {formatDateTime(s.started_at)}
                </Link>
              </td>
              <td data-label={c.participant}>{s.participant_token}</td>
              <td data-label={c.flag}><FlagBadge flag={s.attention_flag} /></td>
              <td data-label={c.composite} className="num">{formatNumber(s.composite_score)}</td>
              <td data-label={c.confidence} className="num">{formatPercent(s.composite_confidence)}</td>
              <td data-label={c.quality}>{s.quality_verdict}</td>
              <td data-label={c.duration} className="num">{formatDuration(s.duration_sec)}</td>
              <td data-label={c.bundle}>{s.bundle_version ?? "-"}</td>
              <td data-label={c.reviewed}>{s.reviewed ? "Yes" : "No"}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
