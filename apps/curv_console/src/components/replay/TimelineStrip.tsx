"use client";

import type { MouseEvent } from "react";
import type { GazeEvent, ReplayArea, RibbonSegment } from "@/lib/asd/replay";
import { formatNumber } from "@/lib/format";
import { useLocale, useText } from "@/lib/i18n/provider";
import type { ResearchMessages } from "@/messages/research";
import { roleToken } from "./drawModel";
import s from "./replay.module.css";

const W = 800;
const LABEL_W = 110;
const ROW_H = 16;
const GAP = 6;

/**
 * The replay timeline (ADR-0015 V-1): the AOI-hit ribbon from the Rust core (`replayRibbon`, role colours, hover shows
 * the AOI and its roles), blink and loss from the core's events, the catalog areas scheduled on screen, and the
 * playhead. Clicking seeks (the scrubber is the keyboard route).
 */
export function TimelineStrip({
  clipTitle,
  start,
  end,
  segments,
  events,
  areas,
  tSec,
  onSeek,
  text,
}: {
  clipTitle: string;
  start: number;
  end: number;
  segments: readonly RibbonSegment[];
  events: readonly GazeEvent[];
  areas: readonly ReplayArea[];
  tSec: number;
  onSeek?: (t: number) => void;
  text?: ResearchMessages;
}) {
  const bundle = useText();
  const rp = (text ?? bundle.research).replay;
  const tl = rp.timeline;
  const locale = useLocale();
  const span = end - start || 1;
  const x = (t: number) => LABEL_W + ((Math.min(Math.max(t, start), end) - start) / span) * (W - LABEL_W - 4);
  const sec = (v: number) => formatNumber(v, 2, locale);
  const gaps = events.filter((e) => e.kind === "blink" || e.kind === "lost");
  const rows = { ribbon: 4, loss: 4 + ROW_H + GAP, targets: 4 + 2 * (ROW_H + GAP) };
  const targetH = Math.max(4, Math.floor(ROW_H / Math.max(1, areas.length)));
  const H = rows.targets + Math.max(ROW_H, targetH * areas.length) + 6;

  const seek = (ev: MouseEvent<SVGSVGElement>) => {
    if (!onSeek) return;
    const box = ev.currentTarget.getBoundingClientRect();
    const vx = ((ev.clientX - box.left) / box.width) * W;
    if (vx < LABEL_W) return;
    onSeek(start + ((vx - LABEL_W) / (W - LABEL_W - 4)) * span);
  };

  return (
    <figure className={s.figure}>
      <figcaption>{tl.title}</figcaption>
      <svg
        className={s.strip}
        viewBox={`0 0 ${W} ${H}`}
        role="img"
        aria-label={tl.alt(clipTitle, segments.length)}
        onClick={seek}
        data-testid="replay-timeline"
      >
        <text x={4} y={rows.ribbon + 12}>{tl.ribbon}</text>
        <text x={4} y={rows.loss + 12}>{tl.loss}</text>
        <text x={4} y={rows.targets + 12}>{tl.targets}</text>
        <line className={s.rowLine} x1={LABEL_W} x2={W} y1={rows.loss - GAP / 2} y2={rows.loss - GAP / 2} />
        <line className={s.rowLine} x1={LABEL_W} x2={W} y1={rows.targets - GAP / 2} y2={rows.targets - GAP / 2} />
        <g data-testid="replay-ribbon">
          {segments.map((g) => {
            const cls = !g.valid ? s.fill_invalid : g.aoi ? s[`fill_${roleToken(g.roles)}`] : undefined;
            if (!cls) return null;
            const what = !g.valid ? tl.invalid : `${g.aoi} (${g.roles.length ? g.roles.join(", ") : rp.noRole})`;
            return (
              <rect
                key={`${g.startSec}-${g.endSec}`}
                className={cls}
                x={x(g.startSec)}
                y={rows.ribbon}
                width={Math.max(0.5, x(g.endSec) - x(g.startSec))}
                height={ROW_H}
                data-aoi={g.aoi ?? ""}
              >
                <title>{tl.segment(what, sec(g.startSec), sec(g.endSec))}</title>
              </rect>
            );
          })}
        </g>
        <g>
          {gaps.map((g) => (
            <rect
              key={`${g.kind}-${g.startSec}`}
              className={g.kind === "blink" ? s.fill_blink : s.fill_lost}
              x={x(g.startSec)}
              y={rows.loss}
              width={Math.max(1, x(g.endSec) - x(g.startSec))}
              height={ROW_H}
            >
              <title>{tl.segment(g.kind === "blink" ? rp.blink : rp.lost, sec(g.startSec), sec(g.endSec))}</title>
            </rect>
          ))}
        </g>
        <g>
          {areas.map((a, i) => (
            <rect
              key={a.name}
              className={s.targetBar}
              x={x(a.startSec)}
              y={rows.targets + i * targetH}
              width={Math.max(1, x(a.endSec) - x(a.startSec))}
              height={Math.max(2, targetH - 1)}
            >
              <title>{tl.segment(`${a.name} (${a.roles.length ? a.roles.join(", ") : rp.noRole})`, sec(a.startSec), sec(a.endSec))}</title>
            </rect>
          ))}
        </g>
        <line className={s.playhead} x1={x(tSec)} x2={x(tSec)} y1={0} y2={H} data-testid="replay-playhead">
          <title>{tl.playhead(rp.seconds(sec(tSec)))}</title>
        </line>
      </svg>
      <ul className={s.legend}>
        <li><span className={`${s.swatch} ${s.bg_blink}`} aria-hidden="true" />{rp.blink}</li>
        <li><span className={`${s.swatch} ${s.bg_lost}`} aria-hidden="true" />{rp.lost}</li>
        <li><span className={`${s.swatch} ${s.bg_invalid}`} aria-hidden="true" />{tl.invalid}</li>
      </ul>
    </figure>
  );
}
