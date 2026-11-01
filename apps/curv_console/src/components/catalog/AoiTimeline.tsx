"use client";

import type { MouseEvent } from "react";
import { AOI_ROLES, type CatalogClip } from "@/lib/api/catalog";
import { catalogText } from "@/messages/catalog";
import s from "./catalog.module.css";

const t = catalogText.aoi;

/** CSS class for an AOI's colour: its first known role, else the neutral "no role" colour. */
export function roleClass(roles: string[]): string {
  const r = roles.find((x) => (AOI_ROLES as readonly string[]).includes(x));
  return s[`role-${r ?? "none"}`] ?? "";
}

export function RoleChips({ roles }: { roles: string[] }) {
  if (!roles.length) return <span className="muted">{t.noRole}</span>;
  return (
    <span className={s.chips}>
      {roles.map((r) => (
        <span key={r} className={`${s.chip} ${roleClass([r])}`}>{t.roleNames[r] ?? r}</span>
      ))}
    </span>
  );
}

const pct = (v: number, total: number) => `${Math.max(0, Math.min(100, (v / total) * 100))}%`;

/**
 * One track per AOI with a bar over its time range, a ruler and a playhead. Clicking the ruler moves the playhead,
 * clicking a bar selects the AOI. The range input is the keyboard path for the playhead.
 */
export function AoiTimeline({
  clip,
  selected,
  onSelect,
  time,
  onTime,
}: {
  clip: CatalogClip;
  selected: number | null;
  onSelect: (index: number) => void;
  time: number;
  onTime: (t: number) => void;
}) {
  const duration = Number.isFinite(clip.durationSec) && clip.durationSec > 0 ? clip.durationSec : 1;
  const step = duration > 20 ? 5 : duration > 6 ? 1 : 0.5;
  const ticks = Array.from({ length: Math.floor(duration / step) + 1 }, (_, i) => i * step);
  const seek = (e: MouseEvent<HTMLDivElement>) => {
    const rect = e.currentTarget.getBoundingClientRect();
    if (rect.width <= 0) return;
    onTime(Math.round(((e.clientX - rect.left) / rect.width) * duration * 10) / 10);
  };
  return (
    <section className={s.timeline} aria-label={t.timeline}>
      <label>
        {t.playhead}
        <input
          type="range"
          min={0}
          max={duration}
          step={0.1}
          value={Math.min(time, duration)}
          onChange={(e) => onTime(e.currentTarget.valueAsNumber)}
        />
      </label>
      <span className="muted">{t.at(Math.min(time, duration))}</span>
      <div className={s.ruler} onClick={seek} aria-hidden="true">
        {ticks.map((x) => (
          <span key={x} className={s.tick} style={{ left: pct(x, duration) }}>{x}</span>
        ))}
      </div>
      <div className={s.trackArea}>
        {clip.areas.length === 0 ? <p className="muted">{t.none}</p> : null}
        <ul className={s.tracks}>
          {clip.areas.map((a, i) => (
            <li key={i} className={s.track}>
              <span className={s.trackLabel} title={a.name}>{a.name || "?"}</span>
              <span className={s.lane}>
                <button
                  type="button"
                  className={`${s.bar} ${roleClass(a.roles)}`}
                  style={{ left: pct(a.startSec, duration), width: pct(Math.max(0, a.endSec - a.startSec), duration) }}
                  aria-pressed={selected === i}
                  aria-label={`${a.name || "?"} ${a.startSec}–${a.endSec} s`}
                  onClick={() => onSelect(i)}
                />
              </span>
            </li>
          ))}
        </ul>
        <div className={s.lanesOverlay}>
          <span className={s.playhead} style={{ left: pct(Math.min(time, duration), duration) }} />
        </div>
      </div>
    </section>
  );
}
