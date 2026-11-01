"use client";

import { useActionState, useRef, useState } from "react";
import { MAX_NOTE_CHARS, type ClinicalNote } from "@/lib/api/participant";
import { formatDateTime } from "@/lib/format";
import { participantText } from "@/messages/participant";
import { failureText, type ActionResult } from "./types";

const t = participantText.notes;

type State = ActionResult | null;
type Create = (form: FormData) => Promise<ActionResult>;

export interface SessionOption {
  id: string;
  label: string;
}

/**
 * One form for a new note or, with `corrects`, a correction (NOTE-2: notes are append-only, so a correction is a new
 * note naming the original). Text only goes to the server action; it is never put in a URL or the page title.
 */
function NoteForm({
  create,
  sessions,
  corrects,
  onDone,
}: {
  create: Create;
  sessions: SessionOption[];
  corrects?: ClinicalNote;
  onDone?: () => void;
}) {
  const ref = useRef<HTMLFormElement>(null);
  const [state, action, pending] = useActionState<State, FormData>(async (_prev, form) => {
    const result = await create(form);
    if (result.ok) {
      ref.current?.reset();
      onDone?.();
    }
    return result;
  }, null);
  const id = corrects ? `note-body-${corrects.id}` : "note-body-new";
  return (
    <form ref={ref} action={action} className="stack" aria-label={corrects ? t.correctTitle : t.newTitle}>
      {corrects ? (
        <>
          <input type="hidden" name="corrects_id" value={corrects.id} />
          <p className="muted">{t.correctHelp}</p>
        </>
      ) : (
        <label>
          {t.session}
          <select name="session_id" defaultValue="">
            <option value="">{t.noSession}</option>
            {sessions.map((s) => (
              <option key={s.id} value={s.id}>{s.label}</option>
            ))}
          </select>
        </label>
      )}
      <label htmlFor={id}>{corrects ? t.correctTitle : t.body}</label>
      <textarea id={id} name="body" rows={corrects ? 4 : 5} required maxLength={MAX_NOTE_CHARS} defaultValue={corrects?.body ?? ""} />
      <p className="muted">{t.maxHint(MAX_NOTE_CHARS)}</p>
      <div className="filter-actions">
        <button type="submit" disabled={pending}>
          {pending ? t.adding : corrects ? t.saveCorrection : t.add}
        </button>
        {onDone ? (
          <button type="button" onClick={onDone} disabled={pending}>{t.cancel}</button>
        ) : null}
      </div>
      {state?.ok && !corrects ? <p role="status">{t.saved}</p> : null}
      {state && !state.ok ? <p role="alert" className="error">{t.failed} {failureText(state)}</p> : null}
    </form>
  );
}

function NoteItem({
  note,
  canWrite,
  create,
  sessions,
  corrector,
}: {
  note: ClinicalNote;
  canWrite: boolean;
  create: Create;
  sessions: SessionOption[];
  corrector: ClinicalNote | undefined;
}) {
  const [correcting, setCorrecting] = useState(false);
  const superseded = Boolean(note.corrected_by);
  return (
    <li id={`note-${note.id}`} className="note card" data-kind={note.kind} data-corrected={superseded ? "true" : "false"}>
      <p className="muted note-meta">
        <strong>{t.kindLabel[note.kind]}</strong> · {t.byline(note.author_display_name, formatDateTime(note.created_at))}
        {note.corrects_id ? <> · <a href={`#note-${note.corrects_id}`}>{t.correctionOf}</a></> : null}
        {superseded ? (
          <> · <span className="badge badge-unreliable">{t.corrected}</span>{" "}
            {corrector ? <a href={`#note-${corrector.id}`}>{t.correctedBy}</a> : null}</>
        ) : null}
      </p>
      <p className={superseded ? "note-body struck" : "note-body"}>{superseded ? <s>{note.body}</s> : note.body}</p>
      {canWrite && !superseded ? (
        correcting ? (
          <NoteForm create={create} sessions={sessions} corrects={note} onDone={() => setCorrecting(false)} />
        ) : (
          <button type="button" onClick={() => setCorrecting(true)} aria-label={`${t.correct}: ${formatDateTime(note.created_at)}`}>
            {t.correct}
          </button>
        )
      ) : null}
    </li>
  );
}

export function NotesPanel({
  notes,
  total,
  sessions,
  canWrite,
  create,
}: {
  notes: ClinicalNote[];
  total?: number | null;
  sessions: SessionOption[];
  canWrite: boolean;
  create: Create;
}) {
  const byId = new Map(notes.map((n) => [n.id, n]));
  return (
    <section aria-labelledby="notes-h" className="stack">
      <h2 id="notes-h">{t.title}</h2>
      <p className="muted">{t.intro}</p>
      {canWrite ? (
        <div className="card">
          <h3>{t.newTitle}</h3>
          <NoteForm create={create} sessions={sessions} />
        </div>
      ) : (
        <p className="muted">{t.readOnly}</p>
      )}
      {notes.length === 0 ? (
        <p className="muted">{t.empty}</p>
      ) : (
        <>
          <ul className="notes" aria-label={t.title}>
            {notes.map((n) => (
              <NoteItem
                key={n.id}
                note={n}
                canWrite={canWrite}
                create={create}
                sessions={sessions}
                corrector={n.corrected_by ? byId.get(n.corrected_by) : undefined}
              />
            ))}
          </ul>
          <p className="muted">{t.count(notes.length, total)}</p>
        </>
      )}
    </section>
  );
}
