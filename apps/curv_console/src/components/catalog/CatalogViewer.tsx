"use client";

import { useState } from "react";
import type { StimulusCatalog } from "@/lib/api/catalog";
import { AoiFrame } from "./AoiFrame";
import { AoiTimeline, RoleChips } from "./AoiTimeline";
import { BatteryPanel } from "./BatteryPanel";
import s from "./catalog.module.css";
import { useText } from "@/lib/i18n/provider";


/** Read-only view of a catalog version: batteries, then each clip with its AOI timeline and boxes. */
export function CatalogViewer({ catalog }: { catalog: StimulusCatalog }) {
  const tx = useText();
  const t = tx.catalog;
  const [clipIdx, setClipIdx] = useState(0);
  const [areaIdx, setAreaIdx] = useState<number | null>(null);
  const [time, setTime] = useState(0);
  const clip = catalog.clips[clipIdx];
  const area = clip && areaIdx !== null ? clip.areas[areaIdx] : undefined;

  return (
    <div className="stack">
      <h2 className={s.section}>{t.batteries.title}</h2>
      <BatteryPanel catalog={catalog} />

      <h2 className={s.section}>{t.clips.title}</h2>
      <div className={s.layout}>
        <nav aria-label={t.clips.list}>
          <ul className={s.clipList}>
            {catalog.clips.map((c, i) => (
              <li key={`${c.id}-${i}`}>
                <button
                  type="button"
                  className={s.clipButton}
                  aria-current={clipIdx === i}
                  onClick={() => {
                    setClipIdx(i);
                    setAreaIdx(null);
                    setTime(0);
                  }}
                >
                  <span>{c.title || c.id}</span>
                  <span className={s.clipMeta}>
                    {c.id} · {t.clips.seconds(c.durationSec)} · {t.clips.areas(c.areas.length)}
                  </span>
                </button>
              </li>
            ))}
          </ul>
        </nav>
        {clip ? (
          <section className="panel stack" aria-label={`${t.clips.title}: ${clip.id}`}>
            <h3 style={{ margin: 0 }}>{clip.title || clip.id}</h3>
            <dl className="facts">
              <div><dt>{t.clips.id}</dt><dd>{clip.id}</dd></div>
              <div><dt>{t.clips.category}</dt><dd>{clip.category}</dd></div>
              <div><dt>{t.clips.duration}</dt><dd>{clip.durationSec}</dd></div>
              <div><dt>{t.view.facts.agePacks}</dt><dd>{t.clips.ageRange(clip.minMonths, clip.maxMonths)}</dd></div>
              <div>
                <dt>{t.clips.batteries}</dt>
                <dd>{clip.protocols.length ? clip.protocols.map((m) => `${m.id} #${m.order + 1}`).join(", ") : t.clips.noBattery}</dd>
              </div>
            </dl>
            <AoiTimeline clip={clip} selected={areaIdx} onSelect={setAreaIdx} time={time} onTime={setTime} />
            <AoiFrame clip={clip} time={time} selected={areaIdx} onSelect={setAreaIdx} />
            <div className="table-wrap">
              <table className="data">
                <thead>
                  <tr>
                    <th>{t.aoi.name}</th><th className="num">{t.aoi.start}</th><th className="num">{t.aoi.end}</th>
                    <th className="num">{t.aoi.xMin}</th><th className="num">{t.aoi.yMin}</th>
                    <th className="num">{t.aoi.xMax}</th><th className="num">{t.aoi.yMax}</th><th>{t.aoi.roles}</th>
                  </tr>
                </thead>
                <tbody>
                  {clip.areas.map((a, i) => (
                    <tr key={i} aria-selected={area === a} onClick={() => setAreaIdx(i)}>
                      <td>{a.name}</td><td className="num">{a.startSec}</td><td className="num">{a.endSec}</td>
                      <td className="num">{a.xMin}</td><td className="num">{a.yMin}</td>
                      <td className="num">{a.xMax}</td><td className="num">{a.yMax}</td>
                      <td><RoleChips roles={a.roles} /></td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </section>
        ) : (
          <p className="muted">{t.clips.none}</p>
        )}
      </div>
    </div>
  );
}
