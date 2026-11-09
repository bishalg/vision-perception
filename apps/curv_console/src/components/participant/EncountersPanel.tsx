"use client";

import Link from "next/link";
import { useRouter } from "next/navigation";
import { useState, useTransition } from "react";
import type { Encounter } from "@/lib/api/emr";
import { formatDateTime } from "@/lib/format";
import { useText } from "@/lib/i18n/provider";
import { EncounterForm, type SessionChoice } from "./EncounterForm";
import { failureText, type ActionResult } from "./types";
import { orgPaths } from "@/lib/paths";

/** Server actions of the Visits tab, already bound to the org and participant. */
export interface VisitActions {
  create: (form: FormData) => Promise<ActionResult>;
  update: (encounterId: string, rowVersion: number, form: FormData) => Promise<ActionResult>;
  state: (encounterId: string, rowVersion: number, change: "close" | "reopen" | "follow_up_done") => Promise<ActionResult>;
  session: (encounterId: string, sessionId: string, link: boolean) => Promise<ActionResult>;
}

export interface EncountersPanelProps {
  encounters: Encounter[];
  sessions: SessionChoice[];
  /** `encounter.write`, and the participant has not withdrawn. */
  canWrite: boolean;
  /** `note.write`: the summary field is offered. */
  canSummary: boolean;
  /** `clinical.content`: summaries are shown (the API sends them only then). */
  canContent: boolean;
  /** REC-4: only owner and clinician reopen a closed visit. */
  canReopen: boolean;
  actions: VisitActions;
  /** Request time (ms), so "overdue" is the same on server and client. */
  now: number;
  /** `/org`, for session links. */
  orgPath: string;
}

function VisitItem({
  e,
  p,
  sessionLabel,
}: {
  e: Encounter;
  p: EncountersPanelProps;
  sessionLabel: (id: string) => string;
}) {
  const router = useRouter();
  const text = useText();
  const t = text.participant.visits;
  const kind = text.domain.enumLabel(text.locale, "encounter_kind", e.kind);
  const [editing, setEditing] = useState(false);
  const [busy, start] = useTransition();
  const [result, setResult] = useState<ActionResult | null>(null);
  const [linkId, setLinkId] = useState("");
  const run = (job: () => Promise<ActionResult>) =>
    start(async () => {
      let r: ActionResult;
      try {
        r = await job();
      } catch {
        r = { ok: false, slug: null, status: 0 };
      }
      setResult(r);
    });

  const open = e.status === "open";
  const linked = e.session_ids ?? [];
  const unlinked = p.sessions.filter((s) => !linked.includes(s.id));
  const due = e.follow_up_due_at && !e.follow_up_done_at ? e.follow_up_due_at : null;
  const overdue = due != null && Date.parse(due) < p.now;
  const stale = result && !result.ok && (result.slug === "stale-version" || result.status === 412);

  return (
    <li className="card stack" data-testid="visit" data-status={e.status} aria-label={t.visitLine(kind, formatDateTime(e.occurred_at))}>
      <p className="note-meta">
        <strong>{kind}</strong> · {formatDateTime(e.occurred_at)} ·{" "}
        <span className={`badge ${open ? "badge-green" : "badge-unreliable"}`}>{text.domain.enumLabel(text.locale, "encounter_status", e.status)}</span>
        {e.clinician_display_name ? <span className="muted"> · {t.by}: {e.clinician_display_name}</span> : null}
      </p>
      {e.reason ? <p>{e.reason}</p> : null}
      {p.canContent && e.summary ? (
        <p className="note-body" data-testid="visit-summary">{e.summary}</p>
      ) : e.has_summary && !p.canContent ? (
        <p className="muted">{t.summaryHidden}</p>
      ) : null}
      {due ? (
        <p>
          {t.dueBy}: {formatDateTime(due)} {overdue ? <span className="badge badge-red">{t.overdue}</span> : null}{" "}
          {p.canWrite ? (
            <button type="button" disabled={busy} onClick={() => run(() => p.actions.state(e.id, e.row_version, "follow_up_done"))}>
              {t.markDone}
            </button>
          ) : null}
        </p>
      ) : e.follow_up_done_at ? (
        <p className="muted">{t.done}: {formatDateTime(e.follow_up_done_at)}</p>
      ) : null}
      <div>
        <strong>{t.linked}:</strong>{" "}
        {linked.length === 0 ? (
          <span className="muted">{t.noneLinked}</span>
        ) : (
          <ul className="plain">
            {linked.map((s) => (
              <li key={s}>
                <Link href={orgPaths(p.orgPath).clinic.session(s)}>{sessionLabel(s)}</Link>{" "}
                {p.canWrite && open ? (
                  <button
                    type="button"
                    className="link-button"
                    disabled={busy}
                    aria-label={t.unlinkLabel(sessionLabel(s))}
                    onClick={() => run(() => p.actions.session(e.id, s, false))}
                  >
                    {t.unlink}
                  </button>
                ) : null}
              </li>
            ))}
          </ul>
        )}
      </div>
      {p.canWrite && open && unlinked.length > 0 ? (
        <div className="confirm">
          <label>
            {t.linkSession}{" "}
            <select value={linkId} onChange={(ev) => setLinkId(ev.target.value)}>
              <option value="">{t.chooseSession}</option>
              {unlinked.map((s) => (
                <option key={s.id} value={s.id}>{s.label}</option>
              ))}
            </select>
          </label>
          <button
            type="button"
            className="secondary"
            disabled={busy || !linkId}
            onClick={() => run(async () => {
              const r = await p.actions.session(e.id, linkId, true);
              if (r.ok) setLinkId("");
              return r;
            })}
          >
            {t.link}
          </button>
        </div>
      ) : null}
      {editing ? (
        <EncounterForm
          encounter={e}
          sessions={p.sessions}
          canSummary={p.canSummary}
          submit={(form) => p.actions.update(e.id, e.row_version, form)}
          onDone={() => setEditing(false)}
        />
      ) : p.canWrite ? (
        <div className="dialog-actions">
          {open ? (
            <>
              <button type="button" className="secondary" onClick={() => setEditing(true)} disabled={busy}>{t.edit}</button>
              <button type="button" className="secondary" onClick={() => run(() => p.actions.state(e.id, e.row_version, "close"))} disabled={busy}>
                {t.close}
              </button>
            </>
          ) : p.canReopen ? (
            <button type="button" className="secondary" onClick={() => run(() => p.actions.state(e.id, e.row_version, "reopen"))} disabled={busy}>
              {t.reopen}
            </button>
          ) : null}
        </div>
      ) : null}
      {result && !result.ok ? (
        <div role="alert" className="error">
          <p>{failureText(result, text.locale)}</p>
          {stale ? (
            <button type="button" onClick={() => router.refresh()}>{t.reload}</button>
          ) : null}
        </div>
      ) : null}
    </li>
  );
}

/** Visits tab (REC-4): list newest first, add, change, close/reopen, follow-up done, link sessions. */
export function EncountersPanel(p: EncountersPanelProps) {
  const t = useText().participant.visits;
  const [adding, setAdding] = useState(false);
  const [saved, setSaved] = useState(false);
  const labels = new Map(p.sessions.map((s) => [s.id, s.label]));
  const sessionLabel = (id: string) => labels.get(id) ?? id.slice(0, 8);
  return (
    <section aria-labelledby="visits-h" className="stack">
      <h2 id="visits-h">{t.title}</h2>
      <p className="muted">{t.intro}</p>
      {p.canWrite ? (
        adding ? (
          <EncounterForm
            submit={async (form) => {
              const r = await p.actions.create(form);
              if (r.ok) setSaved(true);
              return r;
            }}
            sessions={p.sessions}
            canSummary={p.canSummary}
            onDone={() => setAdding(false)}
          />
        ) : (
          <div>
            <button type="button" onClick={() => { setSaved(false); setAdding(true); }}>{t.add}</button>
            {saved ? <p role="status">{t.saved}</p> : null}
          </div>
        )
      ) : (
        <p className="muted">{t.readOnly}</p>
      )}
      {p.encounters.length === 0 ? (
        <p className="muted">{t.empty}</p>
      ) : (
        <ul className="notes" aria-label={t.title}>
          {p.encounters.map((e) => (
            <VisitItem key={`${e.id}:${e.row_version}`} e={e} p={p} sessionLabel={sessionLabel} />
          ))}
        </ul>
      )}
    </section>
  );
}
