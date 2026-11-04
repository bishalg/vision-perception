import type { AnalysisSessionRow } from "@/lib/api/research";
import { formatNumber } from "@/lib/format";
import type { ResearchMessages } from "@/messages/research";

/**
 * RUN-3 per-session rows: published and draft composite and flag side by side, with the change. The rows carry no
 * token, MRN, name or device model (PRV-2); a missing value shows a dash, never a made-up number.
 */
export function RunSessionsTable({ rows, text }: { rows: readonly AnalysisSessionRow[]; text: ResearchMessages }) {
  const t = text.runs;
  const dash = text.charts.noTransition;
  const flag = (f: string | null | undefined) => (f ? text.preview.flagLabel[f as keyof typeof text.preview.flagLabel] ?? f : dash);
  const num = (v: number | null | undefined) => (v == null ? dash : formatNumber(v, 3));
  const side = (s: AnalysisSessionRow["published"]) => `${num(s.composite)} (${flag(s.flag)})`;
  if (rows.length === 0) return <p className="muted">{t.noSessions}</p>;
  return (
    <div className="table-wrap">
      <table className="data responsive" aria-label={t.sessionsTitle}>
        <thead>
          <tr>
            <th scope="col">{t.sessionCols.session}</th>
            <th scope="col">{t.sessionCols.level}</th>
            <th scope="col">{t.sessionCols.band}</th>
            <th scope="col">{t.sessionCols.published}</th>
            <th scope="col">{t.sessionCols.draft}</th>
            <th scope="col">{t.sessionCols.delta}</th>
            <th scope="col">{t.sessionCols.warnings}</th>
          </tr>
        </thead>
        <tbody>
          {rows.map((row) => (
            <tr key={row.session_id}>
              <td data-label={t.sessionCols.session}>{row.session_id}</td>
              <td data-label={t.sessionCols.level}>{text.level[row.level]}</td>
              <td data-label={t.sessionCols.band}>{row.age_band ?? dash}</td>
              <td data-label={t.sessionCols.published}>{side(row.published)}</td>
              <td data-label={t.sessionCols.draft}>{side(row.draft)}</td>
              <td data-label={t.sessionCols.delta} className="num">{num(row.delta.composite)}</td>
              <td data-label={t.sessionCols.warnings}>{row.warnings?.length ? row.warnings.join("; ") : dash}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
