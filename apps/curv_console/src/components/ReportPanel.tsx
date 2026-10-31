import { en } from "@/messages/en";
import type { SessionDetail } from "@/lib/api";

/**
 * The server renders the observation diary PDF from the stored record for every live
 * (TrueDepth) capture (WP-B.8); simulated and synthetic sessions are refused (409, SIM-5).
 * `report_pdf_available` only says whether the device also uploaded its own PDF, so it is
 * not the gate for the download link.
 */
export function reportAvailable(session: Pick<SessionDetail, "capture_source" | "report_pdf_available">): boolean {
  return session.capture_source === "truedepth" || session.report_pdf_available === true;
}

export function ReportPanel({ session }: { session: SessionDetail }) {
  const r = en.detail.report;
  return (
    <div className="panel">
      {reportAvailable(session) ? (
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
