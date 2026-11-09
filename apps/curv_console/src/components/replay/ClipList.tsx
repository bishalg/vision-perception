"use client";

import type { ExpectedBand } from "@/lib/asd/replay";
import { formatNumber } from "@/lib/format";
import { useLocale, useText } from "@/lib/i18n/provider";
import { enumLabel } from "@/messages/domain";
import type { ResearchMessages } from "@/messages/research";
import { barGeometry } from "./drawModel";
import s from "./replay.module.css";

export interface ClipEntry {
  id: string;
  title: string;
  durationSec: number | null;
  /** The whole-clip share of gaze on the target AOI (the core's trace over one clip-long window). */
  recorded: number | null;
  /** The first expected band of that metric for the child's bracket. */
  band: ExpectedBand | null;
  /** The scoring status of the clip (worst row), from the stored clip scores. */
  status: string | null;
  aoi: string | null;
}

/**
 * The session's clips in play order (ADR-0015 V-2): recorded versus expected as a dot on a 0..1 track with the
 * expected band, and the status word from scoring next to it (never a colour alone). Selecting a clip loads it.
 */
export function ClipList({
  clips,
  selected,
  onSelect,
  text,
}: {
  clips: readonly ClipEntry[];
  selected: string;
  onSelect: (id: string) => void;
  text?: ResearchMessages;
}) {
  const bundle = useText();
  const rp = (text ?? bundle.research).replay;
  const c = rp.clips;
  const locale = useLocale();
  const n2 = (v: number) => formatNumber(v, 2, locale);
  return (
    <section aria-label={c.title} className={s.stack}>
      <h3 style={{ margin: 0 }}>{c.title}</h3>
      <ol className={s.clipList}>
        {clips.map((clip) => {
          const g = barGeometry(clip.recorded, clip.band);
          const range =
            clip.band && clip.band.low !== null && clip.band.high !== null
              ? `${n2(clip.band.low)}–${n2(clip.band.high)}`
              : clip.band?.low != null
                ? `≥ ${n2(clip.band.low)}`
                : clip.band?.high != null
                  ? `≤ ${n2(clip.band.high)}`
                  : null;
          return (
            <li key={clip.id} className={s.clipItem} aria-current={clip.id === selected ? "true" : undefined} data-testid="replay-clip">
              <button type="button" className={s.clipButton} onClick={() => onSelect(clip.id)} aria-label={c.select(clip.title)}>
                {clip.title}
              </button>
              <div className={s.clipMeta}>
                {clip.durationSec !== null ? <span>{c.duration(n2(clip.durationSec))}</span> : null}
                {clip.aoi ? <span>{clip.aoi}</span> : null}
                <span>
                  <strong>{clip.status ? enumLabel(locale, "expectation_status", clip.status) : c.notScored}</strong>
                </span>
              </div>
              {g ? (
                <div className={s.track} aria-hidden="true">
                  {g.to > g.from ? <span className={s.trackBand} style={{ left: `${g.from}%`, width: `${g.to - g.from}%` }} /> : null}
                  {g.dot !== null ? <span className={s.trackDot} style={{ left: `${g.dot}%` }} /> : null}
                </div>
              ) : null}
              <div className={s.clipMeta}>
                <span>{clip.recorded === null ? `${rp.recorded}: -` : c.recorded(n2(clip.recorded))}</span>
                <span>{range ? c.expected(range) : c.noBand}</span>
              </div>
            </li>
          );
        })}
      </ol>
    </section>
  );
}
