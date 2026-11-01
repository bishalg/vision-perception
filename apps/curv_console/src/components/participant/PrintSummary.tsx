import type { ClinicalReview } from "@/lib/api/clinical";
import type { Participant } from "@/lib/api/admin";
import type { AttentionFlag } from "@/lib/api/types";
import type { CareSummary, ClinicalNote } from "@/lib/api/participant";
import { formatDateTime } from "@/lib/format";
import { en } from "@/messages/en";
import { participantText } from "@/messages/participant";
import { PrintButton } from "./PrintButton";

const t = participantText.print;

export interface PrintRow {
  sessionId: string;
  at: string;
  flag: AttentionFlag | null;
  /** `null`: no clinical review. `undefined`: the caller may not read clinical content. */
  review: ClinicalReview | null | undefined;
}

/**
 * Print-optimised clinical summary (WP-C.14; the PDF route is WP-I.5). `@media print` in participant.css hides the
 * console chrome and the controls and leaves this document. Only signed (final) review content is printed; drafts are
 * labelled, never shown. No MRN is printed: it is not on this page's data path (UI-4).
 */
export function PrintSummary({
  participant,
  rows,
  notes,
  care,
  truncated,
  generatedAt,
}: {
  participant: Participant;
  rows: PrintRow[];
  /** Current (not corrected) notes; empty without clinical-content access. */
  notes: ClinicalNote[] | undefined;
  /** Care-event counts; `undefined` without clinical-content access. */
  care?: CareSummary;
  truncated?: number;
  generatedAt: string;
}) {
  const c = t.cols;
  return (
    <section aria-labelledby="print-h" className="print-summary">
      <div className="no-print stack">
        <h2 id="print-h">{t.title}</h2>
        <p className="muted">{t.intro}</p>
        <div>
          <PrintButton label={t.button} />
        </div>
      </div>
      <article className="print-doc" aria-label={t.heading}>
        <h1 className="print-only">{t.heading}</h1>
        <dl className="facts">
          <div>
            <dt>{t.participant}</dt>
            <dd>{participant.participant_token}</dd>
          </div>
          <div>
            <dt>{participantText.header.ageBracket}</dt>
            <dd>{participant.age_bracket_at_enrol ?? "-"}</dd>
          </div>
          <div>
            <dt>{participantText.header.status}</dt>
            <dd>{participant.withdrawn_at ? participantText.header.withdrawn : participantText.header.active}</dd>
          </div>
          <div>
            <dt>{t.generated}</dt>
            <dd>{formatDateTime(generatedAt)}</dd>
          </div>
        </dl>

        <h3>{t.sessionsHeading}</h3>
        {rows.length === 0 ? (
          <p>{t.noSessions}</p>
        ) : (
          <table className="data print-table">
            <thead>
              <tr>
                <th scope="col">{c.when}</th>
                <th scope="col">{c.flag}</th>
                <th scope="col">{c.review}</th>
                <th scope="col">{c.disposition}</th>
                <th scope="col">{c.impression}</th>
              </tr>
            </thead>
            <tbody>
              {rows.map((r) => (
                <tr key={r.sessionId}>
                  <td>{formatDateTime(r.at)}</td>
                  <td>{r.flag ? en.flags[r.flag] : "-"}</td>
                  <ReviewCells review={r.review} />
                </tr>
              ))}
            </tbody>
          </table>
        )}
        {truncated ? <p className="muted">{t.truncated(truncated)}</p> : null}

        {care ? (
          <>
            <h3>{t.careHeading}</h3>
            <p>{t.careLine(care)}</p>
          </>
        ) : null}

        {notes ? (
          <>
            <h3>{t.notesHeading}</h3>
            {notes.length === 0 ? (
              <p>{t.noNotes}</p>
            ) : (
              <ul className="plain">
                {notes.map((n) => (
                  <li key={n.id} className="print-note">
                    <span className="muted">
                      {formatDateTime(n.created_at)}
                      {n.author_display_name ? `, ${n.author_display_name}` : ""}
                    </span>
                    <p>{n.body}</p>
                  </li>
                ))}
              </ul>
            )}
          </>
        ) : null}
        <p className="print-footer muted">{t.footer}</p>
      </article>
    </section>
  );
}

function ReviewCells({ review }: { review: ClinicalReview | null | undefined }) {
  if (review === undefined) return <td colSpan={3}>{t.hidden}</td>;
  if (review === null) return <td colSpan={3}>{t.notReviewed}</td>;
  if (review.state !== "final") return <td colSpan={3}>{t.draft}</td>;
  const names = review.signatures.map((s) => s.signer_display_name).join(", ");
  const disposition = review.current.disposition;
  return (
    <>
      <td>{names ? t.signedBy(names) : "-"}</td>
      <td>{disposition ? (t.dispositions[disposition] ?? disposition) : "-"}</td>
      <td>{review.current.impression ?? "-"}</td>
    </>
  );
}
