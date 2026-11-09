"use client";

import { useEffect, useRef, useState } from "react";
import type { AsdCore } from "@/lib/asd/core";
import { heatmapCohort, heatmapDifference, replayHeatmap, type CohortHeatmap, type Grid, type GazeFrame, type ReplayArea } from "@/lib/asd/replay";
import { useText } from "@/lib/i18n/provider";
import type { ResearchMessages } from "@/messages/research";
import s from "./replay.module.css";

/** Grid of the session heatmap (spec: 64 x 36, bandwidth 0.04 screen units). */
export const HEATMAP_W = 64;
export const HEATMAP_H = 36;
export const HEATMAP_BANDWIDTH = 0.04;
const CW = 320;
const CH = 180;

function cssVar(el: Element, name: string, fallback: string): string {
  return getComputedStyle(el).getPropertyValue(name).trim() || fallback;
}

/** Paints a grid (row-major from the top-left) over the AOI wireframe; `signed` maps -1..1 to two hues. */
function paintGrid(canvas: HTMLCanvasElement, grid: Grid, areas: readonly ReplayArea[], signed: boolean) {
  const ctx = canvas.getContext("2d");
  if (!ctx) return;
  const pos = cssVar(canvas, "--accent", "#1f5fd1");
  const neg = cssVar(canvas, "--red", "#b3261e");
  ctx.globalAlpha = 1;
  ctx.fillStyle = cssVar(canvas, "--surface", "#fff");
  ctx.fillRect(0, 0, CW, CH);
  const cw = CW / grid.w;
  const ch = CH / grid.h;
  for (let r = 0; r < grid.h; r++) {
    for (let c = 0; c < grid.w; c++) {
      const v = grid.cells[r * grid.w + c] ?? 0;
      if (v === 0) continue;
      ctx.globalAlpha = Math.min(1, Math.abs(v));
      ctx.fillStyle = signed && v < 0 ? neg : pos;
      ctx.fillRect(c * cw, r * ch, cw + 0.5, ch + 0.5);
    }
  }
  ctx.globalAlpha = 1;
  ctx.strokeStyle = cssVar(canvas, "--muted", "#666");
  ctx.lineWidth = 1;
  for (const a of areas) ctx.strokeRect(a.xMin * CW, a.yMin * CH, (a.xMax - a.xMin) * CW, (a.yMax - a.yMin) * CH);
}

function GridCanvas({ grid, areas, signed, label }: { grid: Grid; areas: readonly ReplayArea[]; signed: boolean; label: string }) {
  const ref = useRef<HTMLCanvasElement>(null);
  useEffect(() => {
    if (ref.current) paintGrid(ref.current, grid, areas, signed);
  }, [grid, areas, signed]);
  return <canvas ref={ref} className={s.heatCanvas} width={CW} height={CH} role="img" aria-label={label} />;
}

/**
 * Session gaze density next to the age-band cohort density and their difference (ADR-0015 V-3). The grids are
 * computed in Rust (`replayHeatmap`, `heatmapCohort`, `heatmapDifference`); the cohort obeys suppression and prints
 * n. Without cohort grids (the session is not linked to a run) the cohort and difference panels are replaced by a
 * sentence.
 */
export function HeatmapPair({
  core,
  frames,
  clipId,
  areas,
  cohortGrids,
  text,
}: {
  core: AsdCore;
  frames: readonly GazeFrame[];
  clipId: string;
  areas: readonly ReplayArea[];
  /** Session grids of the age-band cohort from a linked run; null or absent hides the cohort panels. */
  cohortGrids?: readonly Grid[] | null;
  text?: ResearchMessages;
}) {
  const bundle = useText();
  const rp = (text ?? bundle.research).replay;
  const h = rp.heatmap;
  const [session, setSession] = useState<Grid | null>(null);
  const [cohort, setCohort] = useState<CohortHeatmap | null>(null);
  const [diff, setDiff] = useState<Grid | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let live = true;
    void (async () => {
      const g = await replayHeatmap(core, frames, { clipId, w: HEATMAP_W, h: HEATMAP_H, bandwidthNorm: HEATMAP_BANDWIDTH });
      if (!live) return;
      if (!g.ok) {
        setError(g.error.kind === "unavailable" ? rp.coreUnavailable : rp.failed(g.error.message));
        return;
      }
      setError(null);
      setSession(g.result);
      if (!cohortGrids) {
        setCohort(null);
        setDiff(null);
        return;
      }
      const c = await heatmapCohort(core, cohortGrids);
      if (!live || !c.ok) return;
      setCohort(c.result);
      if (c.result.grid) {
        const d = await heatmapDifference(core, g.result, c.result.grid);
        if (live && d.ok) setDiff(d.result);
      } else setDiff(null);
    })();
    return () => {
      live = false;
    };
  }, [core, frames, clipId, cohortGrids, rp]);

  return (
    <section className={s.stack} aria-label={h.title} data-testid="replay-heatmaps">
      <h3 style={{ margin: 0 }}>{h.title}</h3>
      {error ? <p role="alert" className="muted">{error}</p> : null}
      <div className={s.heatGrid}>
        {session ? (
          <figure className={s.figure}>
            <figcaption>{h.session}</figcaption>
            <GridCanvas grid={session} areas={areas} signed={false} label={h.alt(h.session)} />
            <span className="muted">{h.samples(session.nSamples)}</span>
          </figure>
        ) : null}
        {cohort ? (
          <figure className={s.figure}>
            <figcaption>{h.cohort}</figcaption>
            {cohort.grid ? <GridCanvas grid={cohort.grid} areas={areas} signed={false} label={h.alt(h.cohort)} /> : null}
            <span className="muted">{cohort.suppressed ? h.suppressed(cohort.n, cohort.minN) : h.cohortN(cohort.n, cohort.minN)}</span>
          </figure>
        ) : null}
        {diff ? (
          <figure className={s.figure}>
            <figcaption>{h.difference}</figcaption>
            <GridCanvas grid={diff} areas={areas} signed label={h.alt(h.difference)} />
            <span className="muted">{h.diffScale}</span>
          </figure>
        ) : null}
      </div>
      <p className="muted" style={{ margin: 0 }}>{h.scale}</p>
      {!cohortGrids ? <p className="muted" style={{ margin: 0 }}>{rp.noCohort}</p> : null}
    </section>
  );
}
