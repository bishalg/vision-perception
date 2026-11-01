"use client";

import { useState } from "react";
import type { StimulusCatalog } from "@/lib/api/catalog";
import { batteriesOf, move, setBattery } from "@/lib/catalog";
import { catalogText } from "@/messages/catalog";
import s from "./catalog.module.css";

const t = catalogText.batteries;

/**
 * Batteries as ordered clip lists. Read-only without `onChange`; with it, clips can be added, removed and reordered,
 * and the result is written back into the clips' memberships (order = position). The summary line is a count and a
 * duration total for orientation; whether a battery is acceptable is the backend's call.
 */
export function BatteryPanel({
  catalog,
  onChange,
}: {
  catalog: StimulusCatalog;
  onChange?: (next: StimulusCatalog) => void;
}) {
  const [extra, setExtra] = useState<string[]>([]);
  const [newId, setNewId] = useState("");
  const known = batteriesOf(catalog);
  const batteries = [...known, ...extra.filter((id) => !known.some((b) => b.id === id)).map((id) => ({ id, clipIds: [] }))];
  const byId = new Map(catalog.clips.map((c) => [c.id, c]));
  const write = (id: string, clipIds: string[]) => onChange?.(setBattery(catalog, id, clipIds));

  return (
    <section aria-label={t.title}>
      <p className="muted">{t.intro}</p>
      {batteries.length === 0 ? <p className="muted">{t.none}</p> : null}
      <div className={s.batteries}>
        {batteries.map((b) => {
          const total = b.clipIds.reduce((sum, id) => sum + (byId.get(id)?.durationSec ?? 0), 0);
          const addable = catalog.clips.filter((c) => !b.clipIds.includes(c.id));
          return (
            <section key={b.id} className={`panel ${s.battery}`} aria-label={b.id}>
              <h3 style={{ margin: 0 }}>{b.id}</h3>
              <p className="muted" style={{ margin: 0 }}>{t.summary(b.clipIds.length, total)}</p>
              {b.clipIds.length === 0 ? <p className="muted">{t.empty}</p> : null}
              <ol>
                {b.clipIds.map((id, i) => {
                  const clip = byId.get(id);
                  return (
                    <li key={id}>
                      <span className={s.batteryRow}>
                        <span>
                          {clip?.title || id} <span className="muted">({id}, {catalogText.clips.seconds(clip?.durationSec ?? NaN)})</span>
                        </span>
                        {onChange ? (
                          <span className={s.batteryActions}>
                            <button type="button" className={s.iconButton} aria-label={`${t.up}: ${id}`} disabled={i === 0} onClick={() => write(b.id, move(b.clipIds, i, i - 1))}>↑</button>
                            <button type="button" className={s.iconButton} aria-label={`${t.down}: ${id}`} disabled={i === b.clipIds.length - 1} onClick={() => write(b.id, move(b.clipIds, i, i + 1))}>↓</button>
                            <button type="button" className={s.iconButton} aria-label={`${t.remove}: ${id}`} onClick={() => write(b.id, b.clipIds.filter((x) => x !== id))}>✕</button>
                          </span>
                        ) : null}
                      </span>
                    </li>
                  );
                })}
              </ol>
              {onChange && addable.length ? (
                <label>
                  {t.addClipTo(b.id)}
                  <select
                    value=""
                    onChange={(e) => {
                      if (e.currentTarget.value) write(b.id, [...b.clipIds, e.currentTarget.value]);
                    }}
                  >
                    <option value="">{t.choose}</option>
                    {addable.map((c) => (
                      <option key={c.id} value={c.id}>{c.title || c.id} ({c.id})</option>
                    ))}
                  </select>
                </label>
              ) : null}
            </section>
          );
        })}
      </div>
      {onChange ? (
        <div className={s.row} style={{ marginTop: ".75rem" }}>
          <label>
            {t.newId}
            <input value={newId} onChange={(e) => setNewId(e.currentTarget.value)} placeholder="express_30s" />
          </label>
          <button
            type="button"
            className={s.small}
            disabled={!newId.trim() || batteries.some((b) => b.id === newId.trim())}
            onClick={() => {
              setExtra((x) => [...x, newId.trim()]);
              setNewId("");
            }}
          >
            {t.add}
          </button>
        </div>
      ) : null}
    </section>
  );
}
