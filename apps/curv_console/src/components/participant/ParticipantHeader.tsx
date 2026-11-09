import { caseSessionsAction, createCaseShareAction } from "@/app/[org]/clinic/participants/[id]/share-actions";
import type { Participant } from "@/lib/api/admin";
import { currentRole } from "@/lib/auth/server";
import { formatDateTime } from "@/lib/format";
import { can } from "@/lib/permissions";
import { getText } from "@/lib/i18n/server";
import { ShareCaseDialog } from "./ShareCaseDialog";
import { ageBracketLabel, languageLabel } from "./labels";
import { orgBase } from "@/lib/paths";

/**
 * Token, age bracket, withdrawn state and, for roles that may see it, whether an MRN is on file. The MRN itself is
 * never in the page data: it is decrypted (and audited) only when the MRN tab's "Show MRN" is pressed.
 *
 * WP-C.19 (SHR-10): the "Share case" action, for roles with `case.share` on a participant who has not withdrawn. The
 * header reads the role itself so the workspace page passes only the participant.
 */
export async function ParticipantHeader({ participant: p }: { participant: Participant }) {
  const role = await currentRole();
  const text = await getText();
  const t = text.participant.header;
  const canShare = can(role, "case.share") && !p.withdrawn_at;
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
          <dd>{p.age_bracket_at_enrol ? ageBracketLabel(text.locale, p.age_bracket_at_enrol) : "-"}</dd>
        </div>
        <div>
          <dt>{t.language}</dt>
          <dd>{p.language ? languageLabel(text.locale, p.language) : "-"}</dd>
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
      {canShare ? (
        <ShareCaseDialog
          canIdentified={can(role, "export.identified")}
          create={createCaseShareAction.bind(null, p.org_id, p.id)}
          loadSessions={caseSessionsAction.bind(null, p.org_id, p.id)}
          orgPath={orgBase(p.org_id)}
        />
      ) : null}
    </header>
  );
}
