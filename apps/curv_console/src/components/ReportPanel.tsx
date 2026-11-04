"use client";

import type { SessionDetail } from "@/lib/api";
import { flagAssessmentText } from "@/lib/flagAssessment";
import { useText } from "@/lib/i18n/provider";
import { notScoredLabel } from "./ScoringStatus";

/**
 * The server renders the observation diary PDF from the stored record for every live
 * (TrueDepth) capture (WP-B.8); simulated and synthetic sessions are refused (409, SIM-5).
 * `report_pdf_available` only says whether the device also uploaded its own PDF, so it is
 * not the gate for the download link.
 */
export function reportAvailable(session: Pick<SessionDetail, "capture_source" | "report_pdf_available">): boolean {
  return session.capture_source === "truedepth" || session.report_pdf_available === true;
}

export function ReportPanel({
  session,
  assessment,
}: {
  session: SessionDetail;
  /** The headline and detail the device sent with the flag, when the API carries them. */
  assessment?: { headline: string; detail?: string | null };
}) {
  const text = useText();
  const t = text.app;
  const r = t.detail.report;
  const notScored = notScoredLabel(session, t);
  const flagText = assessment ? flagAssessmentText(text.locale, assessment) : null;
  return (
    <div className="panel">
      {notScored ? (
        <p role="status">
          <strong>{t.detail.scoring.reportLead(notScored)}</strong> {t.detail.scoring.reportNote}
        </p>
      ) : null}
      {flagText ? (
        <p>
          <strong>{flagText.headline}</strong>
          {flagText.detail ? <> {flagText.detail}</> : null}
        </p>
      ) : null}
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
