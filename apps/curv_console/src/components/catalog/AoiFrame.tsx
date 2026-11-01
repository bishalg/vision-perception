"use client";

import { useRef, type PointerEvent } from "react";
import type { Area, CatalogClip } from "@/lib/api/catalog";
import { areasAt, moveBox, resizeBox } from "@/lib/catalog";
import { catalogText } from "@/messages/catalog";
import { roleClass } from "./AoiTimeline";
import s from "./catalog.module.css";

const t = catalogText.aoi;

type Drag = { index: number; mode: "move" | "resize"; x: number; y: number; start: Area };

/**
 * The AOIs active at the playhead, drawn over a 16:9 placeholder frame (the console has no media preview yet).
 * With `onChange`, the selected box can be dragged (move) or resized from its corner; coordinates stay normalised.
 */
export function AoiFrame({
  clip,
  time,
  selected,
  onSelect,
  onChange,
}: {
  clip: CatalogClip;
  time: number;
  selected: number | null;
  onSelect: (index: number) => void;
  onChange?: (index: number, area: Area) => void;
}) {
  const frame = useRef<HTMLDivElement>(null);
  const drag = useRef<Drag | null>(null);
  const active = areasAt(clip, time);

  const begin = (e: PointerEvent<HTMLElement>, index: number, mode: Drag["mode"]) => {
    onSelect(index);
    if (!onChange) return;
    e.stopPropagation();
    e.currentTarget.setPointerCapture?.(e.pointerId);
    drag.current = { index, mode, x: e.clientX, y: e.clientY, start: clip.areas[index]! };
  };
  const moveTo = (e: PointerEvent<HTMLElement>) => {
    const d = drag.current;
    const rect = frame.current?.getBoundingClientRect();
    if (!d || !onChange || !rect || rect.width <= 0 || rect.height <= 0) return;
    const dx = (e.clientX - d.x) / rect.width;
    const dy = (e.clientY - d.y) / rect.height;
    onChange(d.index, d.mode === "move" ? moveBox(d.start, dx, dy) : resizeBox(d.start, dx, dy));
  };
  const end = () => {
    drag.current = null;
  };

  return (
    <figure style={{ margin: 0 }}>
      <div ref={frame} className={s.frame} role="group" aria-label={t.frame} onPointerMove={moveTo} onPointerUp={end} onPointerCancel={end}>
        {active.map((i) => {
          const a = clip.areas[i]!;
          return (
            <button
              key={i}
              type="button"
              className={`${s.box} ${roleClass(a.roles)} ${onChange ? "" : s.boxReadOnly}`}
              style={{
                left: `${a.xMin * 100}%`,
                top: `${a.yMin * 100}%`,
                width: `${Math.max(0, a.xMax - a.xMin) * 100}%`,
                height: `${Math.max(0, a.yMax - a.yMin) * 100}%`,
              }}
              aria-pressed={selected === i}
              aria-label={`${t.frame}: ${a.name || "?"}`}
              onPointerDown={(e) => begin(e, i, "move")}
              onPointerMove={moveTo}
              onPointerUp={end}
              onClick={() => onSelect(i)}
            >
              <span className={s.boxName}><span>{a.name}</span></span>
              {onChange && selected === i ? (
                <span
                  className={s.handle}
                  data-testid="aoi-resize-handle"
                  onPointerDown={(e) => begin(e, i, "resize")}
                  onPointerMove={moveTo}
                  onPointerUp={end}
                />
              ) : null}
            </button>
          );
        })}
        <span className={s.frameLabel}>{clip.id} · {t.at(time)}</span>
      </div>
      <figcaption className="muted">
        {active.length === 0 ? `${t.noneAtTime} ` : ""}
        {onChange ? t.frameHint : catalogText.clips.media}
      </figcaption>
    </figure>
  );
}
