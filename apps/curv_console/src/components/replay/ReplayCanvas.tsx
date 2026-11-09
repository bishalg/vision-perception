"use client";

import { useCallback, useEffect, useMemo, useRef, useState, type KeyboardEvent } from "react";
import type { GazeEvent, ReplayArea, RibbonSegment } from "@/lib/asd/replay";
import { formatNumber } from "@/lib/format";
import { useLocale, useText } from "@/lib/i18n/provider";
import type { ResearchMessages } from "@/messages/research";
import { ROLE_TOKENS, SPEEDS, clipSpan, drawOps, segmentAt, stepFrame, type DrawOp, type RoleToken, type Sample } from "./drawModel";
import s from "./replay.module.css";

/** Intrinsic canvas size (16:9); CSS scales it. */
const CW = 640;
const CH = 360;

export interface ReplayCanvasProps {
  clipId: string;
  clipTitle: string;
  areas: readonly ReplayArea[];
  events: readonly GazeEvent[];
  samples: readonly Sample[];
  segments: readonly RibbonSegment[];
  tSec: number;
  onSeek: (tSec: number) => void;
  /** Clip switcher entries; omitted hides the switcher. */
  clips?: readonly { id: string; title: string }[];
  onClip?: (id: string) => void;
  /** false: the canvas only (the workbench's second canvas shares the first one's controls). */
  controls?: boolean;
  /** Distinguishes two canvases on one page (labels, ids). */
  caption?: string;
  text?: ResearchMessages;
}

function cssVar(el: Element, name: string, fallback: string): string {
  const v = getComputedStyle(el).getPropertyValue(name).trim();
  return v || fallback;
}

function paint(canvas: HTMLCanvasElement, ops: readonly DrawOp[], title: string) {
  const ctx = canvas.getContext("2d");
  if (!ctx) return;
  const dpr = typeof window === "undefined" ? 1 : Math.min(window.devicePixelRatio || 1, 2);
  if (canvas.width !== CW * dpr) {
    canvas.width = CW * dpr;
    canvas.height = CH * dpr;
  }
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  const surface = cssVar(canvas, "--surface", "#fff");
  const text = cssVar(canvas, "--text", "#111");
  const muted = cssVar(canvas, "--muted", "#666");
  const grey = cssVar(canvas, "--grey", "#888");
  const accent = cssVar(canvas, "--accent", "#1f5fd1");
  const roles = Object.fromEntries(
    [...ROLE_TOKENS, "none"].map((r) => [r, cssVar(canvas, `--role-${r}`, grey)]),
  ) as Record<RoleToken, string>;
  ctx.globalAlpha = 1;
  ctx.fillStyle = surface;
  ctx.fillRect(0, 0, CW, CH);
  // Wireframe frame and the clip title (no media yet, V-8).
  ctx.font = "12px system-ui, sans-serif";
  ctx.fillStyle = muted;
  ctx.textBaseline = "top";
  ctx.fillText(title, 8, 6);
  for (const op of ops) {
    switch (op.op) {
      case "aoi": {
        const c = roles[op.role];
        ctx.globalAlpha = op.hit ? 0.24 : 0.12;
        ctx.fillStyle = c;
        ctx.fillRect(op.x, op.y, op.w, op.h);
        ctx.globalAlpha = 1;
        ctx.strokeStyle = c;
        ctx.lineWidth = op.hit ? 2 : 1;
        ctx.strokeRect(op.x + 0.5, op.y + 0.5, op.w - 1, op.h - 1);
        ctx.fillStyle = c;
        ctx.fillText(op.label, op.x + 4, op.y + 4);
        break;
      }
      case "saccade":
        ctx.globalAlpha = 0.7 * op.alpha;
        ctx.strokeStyle = muted;
        ctx.lineWidth = 1.5;
        ctx.beginPath();
        ctx.moveTo(op.x1, op.y1);
        ctx.lineTo(op.x2, op.y2);
        ctx.stroke();
        break;
      case "fixation":
        ctx.globalAlpha = op.alpha;
        ctx.strokeStyle = accent;
        ctx.lineWidth = op.current ? 2.5 : 1.5;
        ctx.beginPath();
        ctx.arc(op.x, op.y, op.r, 0, Math.PI * 2);
        ctx.stroke();
        break;
      case "trail":
        ctx.globalAlpha = 0.6 * op.alpha;
        ctx.fillStyle = op.grey ? grey : text;
        ctx.beginPath();
        ctx.arc(op.x, op.y, 2.5, 0, Math.PI * 2);
        ctx.fill();
        break;
      case "gaze":
        ctx.globalAlpha = 1;
        ctx.fillStyle = text;
        ctx.strokeStyle = surface;
        ctx.lineWidth = 2;
        ctx.beginPath();
        ctx.arc(op.x, op.y, op.r, 0, Math.PI * 2);
        ctx.fill();
        ctx.stroke();
        break;
      case "gap":
        ctx.globalAlpha = 0.8;
        ctx.strokeStyle = grey;
        ctx.lineWidth = 2;
        ctx.setLineDash([3, 3]);
        ctx.beginPath();
        ctx.arc(op.x, op.y, 9, 0, Math.PI * 2);
        ctx.stroke();
        ctx.setLineDash([]);
        break;
    }
  }
  ctx.globalAlpha = 1;
}

/**
 * The replay canvas (ADR-0015 V-1): AOI boxes active at the playhead, the gaze point with a 300 ms trail, fixations
 * sized by duration, saccade lines, blink and loss greyed. Everything drawn comes from the Rust core's events and
 * ribbon (`drawModel` turns them into ops). Keyboard: Space plays or pauses, the arrows step one frame. Never
 * autoplays (`prefers-reduced-motion` needs nothing more: nothing moves until the viewer presses Play).
 */
export function ReplayCanvas(props: ReplayCanvasProps) {
  const { clipId, clipTitle, areas, events, samples, segments, tSec, onSeek, clips, onClip, controls = true, caption } = props;
  const bundle = useText();
  const t = props.text ?? bundle.research;
  const rp = t.replay;
  const locale = useLocale();
  const canvasRef = useRef<HTMLCanvasElement>(null);
  // Playback belongs to one clip: switching clips stops it without an effect.
  const [playingClip, setPlayingClip] = useState<string | null>(null);
  const playing = playingClip === clipId;
  const setPlaying = useCallback(
    (v: boolean | ((p: boolean) => boolean)) =>
      setPlayingClip((cur) => {
        const was = cur === clipId;
        return (typeof v === "function" ? v(was) : v) ? clipId : null;
      }),
    [clipId],
  );
  const [speed, setSpeed] = useState<number>(1);
  const span = useMemo(() => clipSpan(samples), [samples]);
  const seg = segmentAt(segments, tSec);
  const ops = useMemo(
    () => drawOps({ areas, events, samples, tSec, width: CW, height: CH, hits: seg?.hits ?? [] }),
    [areas, events, samples, tSec, seg],
  );

  useEffect(() => {
    if (canvasRef.current) paint(canvasRef.current, ops, clipTitle);
  }, [ops, clipTitle]);

  // Playback: advance the shared playhead on animation frames; stop at the clip end.
  const tRef = useRef(tSec);
  useEffect(() => {
    tRef.current = tSec;
  }, [tSec]);
  useEffect(() => {
    if (!playing) return;
    let raf = 0;
    let last: number | null = null;
    const tick = (now: number) => {
      const dt = last === null ? 0 : (now - last) / 1000;
      last = now;
      const next = tRef.current + dt * speed;
      if (next >= span.end) {
        onSeek(span.end);
        setPlaying(false);
        return;
      }
      tRef.current = next;
      onSeek(next);
      raf = requestAnimationFrame(tick);
    };
    raf = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(raf);
  }, [playing, speed, span.end, onSeek, setPlaying]);

  const toggle = () => {
    if (!playing && tRef.current >= span.end) onSeek(span.start);
    setPlaying((p) => !p);
  };
  const step = (dir: 1 | -1) => {
    setPlaying(false);
    onSeek(stepFrame(samples, tRef.current, dir));
  };
  const onKey = (ev: KeyboardEvent) => {
    // Form controls keep their own keys (a focused button's Space is its click).
    const tag = (ev.target as HTMLElement).tagName;
    if (tag === "BUTTON" || tag === "SELECT" || tag === "INPUT" || tag === "TEXTAREA") return;
    if (ev.key === " " || ev.key === "Spacebar") {
      ev.preventDefault();
      toggle();
    } else if (ev.key === "ArrowLeft") {
      ev.preventDefault();
      step(-1);
    } else if (ev.key === "ArrowRight") {
      ev.preventDefault();
      step(1);
    }
  };

  const time = rp.seconds(formatNumber(tSec, 2, locale));
  const aoiText = seg
    ? seg.valid
      ? seg.aoi
        ? `${seg.aoi} (${seg.roles.length ? seg.roles.join(", ") : rp.noRole})`
        : rp.noAoi
      : rp.timeline.invalid
    : rp.noAoi;
  const label = rp.canvasLabel(clipTitle, time, aoiText);
  const legendRoles = Array.from(new Set(areas.map((a) => a.roles[0] ?? "none")));

  return (
    <div className={s.stack} onKeyDown={onKey}>
      {caption ? <h3 style={{ margin: 0 }}>{caption}</h3> : null}
      <div className={s.canvasWrap}>
        <canvas
          ref={canvasRef}
          className={s.canvas}
          width={CW}
          height={CH}
          role="img"
          aria-label={label}
          tabIndex={0}
          data-testid="replay-canvas"
          data-clip={clipId}
          data-time={tSec.toFixed(3)}
        />
      </div>
      {controls ? (
        <div className={s.controls}>
          <button type="button" onClick={toggle} aria-pressed={playing}>
            {playing ? rp.pause : rp.play}
          </button>
          <button type="button" onClick={() => step(-1)}>{rp.stepBack}</button>
          <button type="button" onClick={() => step(1)}>{rp.stepForward}</button>
          <input
              type="range"
              aria-label={rp.scrubber}
              min={span.start}
              max={span.end}
              step={0.001}
              value={Math.min(Math.max(tSec, span.start), span.end)}
              onChange={(ev) => {
                setPlaying(false);
                onSeek(Number(ev.currentTarget.value));
              }}
              onKeyDown={(ev) => {
                // Space on the scrubber plays like everywhere else in the player; arrows keep the native step.
                if (ev.key === " ") {
                  ev.preventDefault();
                  ev.stopPropagation();
                  toggle();
                } else if (ev.key === "ArrowLeft" || ev.key === "ArrowRight") {
                  ev.preventDefault();
                  ev.stopPropagation();
                  step(ev.key === "ArrowLeft" ? -1 : 1);
                }
              }}
            />
          <span className={s.time} aria-hidden="true">{time}</span>
          <label>
            {rp.speed}{" "}
            <select value={speed} onChange={(ev) => setSpeed(Number(ev.currentTarget.value))}>
              {SPEEDS.map((x) => (
                <option key={x} value={x}>{rp.speedValue(x)}</option>
              ))}
            </select>
          </label>
          {clips && onClip && clips.length > 1 ? (
            <label>
              {rp.clip}{" "}
              <select value={clipId} onChange={(ev) => onClip(ev.currentTarget.value)}>
                {clips.map((c) => (
                  <option key={c.id} value={c.id}>{c.title}</option>
                ))}
              </select>
            </label>
          ) : null}
        </div>
      ) : null}
      <p className={s.status} aria-live="polite">
        {playing ? null : label}
      </p>
      <ul className={s.legend} aria-label={rp.legend}>
        {legendRoles.map((r) => (
          <li key={r}>
            <span className={`${s.swatch} ${s[`role_${(ROLE_TOKENS as readonly string[]).includes(r) ? r : "none"}`]}`} aria-hidden="true" />
            {r === "none" ? rp.noRole : r}
          </li>
        ))}
        <li><span className={`${s.dot} ${s.gazeDot}`} aria-hidden="true" />{rp.gaze}</li>
        <li>{rp.trail}</li>
        <li>{rp.fixation}</li>
        <li>{rp.saccade}</li>
        <li><span className={`${s.dot} ${s.greyDot}`} aria-hidden="true" />{rp.blinkAndLost}</li>
      </ul>
      {controls ? <p className="muted" style={{ margin: 0 }}>{rp.keys}</p> : null}
    </div>
  );
}
