import Link from "next/link";
import { ATTENTION_FLAGS } from "@/lib/api";
import { CLINICAL_STATES, WORKLIST_ASSIGNED, type WorklistItem } from "@/lib/api/clinical";
import { en } from "@/messages/en";
import { clinicalText } from "@/messages/clinical";
import { FlagBadge } from "@/components/FlagBadge";

const t = clinicalText.worklist;

/**
 * Worklist rows. Links carry only the org and session ids: no patient reference, impression or free text is ever put
 * in an href (UI-4). The "Patient ref" column says whether one is linked, never what it is.
 */
export function WorklistTable({ orgId, items }: { orgId: string; items: WorklistItem[] }) {
  if (items.length === 0) return <p className="muted">{t.empty}</p>;
  const base = `/${encodeURIComponent(orgId)}/sessions`;
  return (
    <div className="table-wrap">
      <table className="data responsive worklist-table">
        <thead>
          <tr>
            <th>{t.cols.received}</th>
            <th>{t.cols.participant}</th>
            <th>{t.cols.flag}</th>
            <th>{t.cols.quality}</th>
            <th>{t.cols.state}</th>
            <th>{t.cols.assigned}</th>
            <th>{t.cols.cosign}</th>
            <th>{t.cols.mrn}</th>
          </tr>
        </thead>
        <tbody>
          {items.map((w) => (
            <tr key={w.id}>
              <td data-label={t.cols.received}>{t.hours(w.age_hours)}</td>
              <td data-label={t.cols.participant}>
                <Link href={`${base}/${encodeURIComponent(w.id)}?tab=clinical`}>{w.participant_token}</Link>
              </td>
              <td data-label={t.cols.flag}>
                <FlagBadge flag={w.attention_flag} />
              </td>
              <td data-label={t.cols.quality}>{w.quality_verdict}</td>
              <td data-label={t.cols.state}>{t.states[w.clinical_state ?? "none"]}</td>
              <td data-label={t.cols.assigned}>{w.assigned_to ? t.assignedOther : "-"}</td>
              <td data-label={t.cols.cosign}>{w.cosign_required ? t.cosignRequired : "-"}</td>
              <td data-label={t.cols.mrn}>{w.has_patient_ref ? t.mrnLinked : "-"}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

/** GET form: filters live in the URL as enum values only. */
export function WorklistFilters({
  state,
  flag,
  assigned,
}: {
  state?: string;
  flag?: string;
  assigned?: string;
}) {
  const f = t.filters;
  return (
    <form method="get" className="picker" aria-label={f.label}>
      <label>
        {f.state}
        <select name="state" defaultValue={state ?? ""}>
          <option value="">{f.any}</option>
          {CLINICAL_STATES.filter((s) => s !== "final").map((s) => (
            <option key={s} value={s}>
              {t.states[s]}
            </option>
          ))}
        </select>
      </label>
      <label>
        {f.flag}
        <select name="flag" defaultValue={flag ?? ""}>
          <option value="">{f.any}</option>
          {ATTENTION_FLAGS.map((x) => (
            <option key={x} value={x}>
              {en.flags[x]}
            </option>
          ))}
        </select>
      </label>
      <label>
        {f.assigned}
        <select name="assigned" defaultValue={assigned ?? ""}>
          <option value="">{f.any}</option>
          {WORKLIST_ASSIGNED.map((a) => (
            <option key={a} value={a}>
              {t.assigned[a]}
            </option>
          ))}
        </select>
      </label>
      <button type="submit">{f.apply}</button>
    </form>
  );
}
