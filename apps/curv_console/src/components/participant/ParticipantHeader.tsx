import type { Participant } from "@/lib/api/admin";
import { formatDateTime } from "@/lib/format";
import { participantText } from "@/messages/participant";

const t = participantText.header;

/**
 * Token, age bracket, withdrawn state and, for roles that may see it, whether an MRN is on file. The MRN itself is
 * never in the page data: it is decrypted (and audited) only when the MRN tab's "Show MRN" is pressed.
 */
export function ParticipantHeader({ participant: p }: { participant: Participant }) {
  return (
    <header className="participant-header">
      <h1>
        {p.participant_token}{" "}
        <span className={`badge ${p.withdrawn_at ? "badge-unreliable" : "badge-green"}`}>
          {p.withdrawn_at ? t.withdrawn : t.active}
        </span>
      </h1>
      {p.withdrawn_at ? <p role="status" className="muted">{t.withdrawnNote}</p> : null}
      <dl className="facts">
        <div>
          <dt>{t.ageBracket}</dt>
          <dd>{p.age_bracket_at_enrol ?? "-"}</dd>
        </div>
        <div>
          <dt>{t.language}</dt>
          <dd>{p.language ?? "-"}</dd>
        </div>
        <div>
          <dt>{t.sessions}</dt>
          <dd>{p.session_count}</dd>
        </div>
        <div>
          <dt>{t.lastSession}</dt>
          <dd>{p.last_session_at ? formatDateTime(p.last_session_at) : "-"}</dd>
        </div>
        {p.has_patient_ref != null ? (
          <div>
            <dt>{t.mrn}</dt>
            <dd>{p.has_patient_ref ? t.mrnOnFile : t.mrnNone}</dd>
          </div>
        ) : null}
      </dl>
    </header>
  );
}
