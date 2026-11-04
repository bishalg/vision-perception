"use client";

import { formatDateTime } from "@/lib/format";
import { useText } from "@/lib/i18n/provider";
import type { ResearchMessages } from "@/messages/research";
import r from "./research.module.css";

export interface PickerSession {
  id: string;
  token: string;
  startedAt: string;
  /** RFL-6: a sealed research frame log is stored for the session. */
  framesAvailable: boolean;
}

/**
 * The recent sessions to preview (RUI-3): a checkbox per session with the "frames" badge, an age field per session
 * (sessions do not carry the child's age) and the selection limit.
 */
export function SessionPicker({
  sessions,
  selected,
  onToggle,
  ages,
  defaultAge,
  onAge,
  onDefaultAge,
  max,
  text: textProp,
}: {
  sessions: PickerSession[];
  selected: ReadonlySet<string>;
  onToggle: (id: string) => void;
  ages: Record<string, number>;
  defaultAge: number;
  onAge: (id: string, age: number) => void;
  onDefaultAge: (age: number) => void;
  max: number;
  text?: ResearchMessages;
}) {
  const bundle = useText();
  const text = textProp ?? bundle.research;
  const p = text.preview;
  const full = selected.size >= max;
  return (
    <fieldset className="stack" style={{ border: 0, padding: 0, margin: 0 }}>
      <legend>{p.pickLegend}</legend>
      <div className={r.row}>
        <label>
          {p.defaultAge}
          <input type="number" min={0} className={r.ageInput} value={defaultAge} onChange={(ev) => onDefaultAge(Math.max(0, Math.round(ev.currentTarget.valueAsNumber || 0)))} />
        </label>
        <span className="muted" role="status">{p.selectedCount(selected.size, max)}{full ? ` ${p.maxReached(max)}` : ""}</span>
      </div>
      <p className="muted" style={{ margin: 0 }}>{p.ageHint}</p>
      {sessions.length === 0 ? <p className="muted">{p.noSessions}</p> : null}
      <div className={r.sessionList}>
        {sessions.map((s) => {
          const checked = selected.has(s.id);
          return (
            <div key={s.id} className={r.sessionRow}>
              <label>
                <input type="checkbox" checked={checked} disabled={!checked && full} onChange={() => onToggle(s.id)} />{" "}
                {s.token} <span className="muted">{formatDateTime(s.startedAt)}</span>
              </label>
              <span className={`badge ${r.badge}`}>{s.framesAvailable ? p.framesBadge : p.scoreBadge}</span>
              {checked ? (
                <input
                  type="number"
                  min={0}
                  className={r.ageInput}
                  aria-label={p.age(s.id)}
                  value={ages[s.id] ?? defaultAge}
                  onChange={(ev) => onAge(s.id, Math.max(0, Math.round(ev.currentTarget.valueAsNumber || 0)))}
                />
              ) : null}
            </div>
          );
        })}
      </div>
    </fieldset>
  );
}
