import { en } from "@/messages/en";
import type { SessionDetail } from "@/lib/api";

export function ReportPanel({ session }: { session: SessionDetail }) {
  const r = en.detail.report;
  return (
    <div className="panel">
      {session.report_pdf_available ? (
        <a className="button" href={`/api/report/${encodeURIComponent(session.id)}`}>
          {r.download}
        </a>
      ) : (
        <p className="muted">{r.unavailable}</p>
      )}
      <p className="muted">{r.note}</p>
    </div>
  );
}
