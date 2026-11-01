import type { Participant } from "@/lib/api";
import { formatDateTime } from "@/lib/format";
import { adminText } from "@/messages/admin";

const t = adminText.participants;
const c = t.cols;

/** Read-only list. `has_patient_ref` is absent for callers without `PatientRefRead`, so that column is then hidden. */
export function ParticipantsTable({ participants }: { participants: Participant[] }) {
  if (participants.length === 0) return <p className="muted">{t.empty}</p>;
  const showRef = participants.some((p) => p.has_patient_ref != null);
  return (
    <div className="table-wrap">
      <table className="data responsive">
        <thead>
          <tr>
            <th scope="col">{c.token}</th>
            <th scope="col">{c.age}</th>
            <th scope="col">{c.language}</th>
            <th scope="col">{c.sessions}</th>
            <th scope="col">{c.lastSession}</th>
            <th scope="col">{c.status}</th>
            {showRef ? <th scope="col">{c.patientRef}</th> : null}
          </tr>
        </thead>
        <tbody>
          {participants.map((p) => (
            <tr key={p.id}>
              <td data-label={c.token}>{p.participant_token}</td>
              <td data-label={c.age}>{p.age_bracket_at_enrol ?? "-"}</td>
              <td data-label={c.language}>{p.language ?? "-"}</td>
              <td data-label={c.sessions}>{p.session_count}</td>
              <td data-label={c.lastSession}>{p.last_session_at ? formatDateTime(p.last_session_at) : "-"}</td>
              <td data-label={c.status}>
                <span className={`badge ${p.withdrawn_at ? "badge-unreliable" : "badge-green"}`}>
                  {p.withdrawn_at ? t.withdrawn : t.active}
                </span>
              </td>
              {showRef ? <td data-label={c.patientRef}>{p.has_patient_ref ? t.refOnFile : t.refNone}</td> : null}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
