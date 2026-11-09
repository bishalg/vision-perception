"use client";

import { useState, type MouseEvent } from "react";
import type { ReplayTrace, TracePoint } from "@/lib/asd/replay";
import { formatNumber } from "@/lib/format";
import { useLocale, useText } from "@/lib/i18n/provider";
import type { ResearchMessages } from "@/messages/research";
import { bandRects, nearestPoint, tracePath, xOf, yOf, type ChartBox } from "./drawModel";
import { bandLabel, traceCsv, traceCsvName } from "./traceData";
import s from "./replay.module.css";

const BOX: Omit<ChartBox, "t0" | "t1"> = { width: 640, height: 240, pad: { l: 44, r: 120, t: 12, b: 34 } };

/**
 * Expected versus actual on the time axis (ADR-0015 V-2): the core's windowed trace of the share of gaze on the
 * clip's target AOI, the published expected band for the child's bracket shaded per segment, its mean dashed and the
 * thresholds as labelled lines (labels from the domain catalogue). Table toggle and CSV of the plotted points (R-15).
 * The axis never implies a diagnosis: it says "share of gaze", "expected range", "recorded".
 */
export function TraceChart({
  clipId,
  aoi,
  trace,
  start,
  end,
  tSec,
  onSeek,
  needAge,
  text,
}: {
  clipId: string;
  aoi: string;
  trace: ReplayTrace | null;
  start: number;
  end: number;
  tSec: number;
  onSeek?: (t: number) => void;
  /** No age: the trace is drawn without bands and a sentence says why. */
  needAge?: boolean;
  text?: ResearchMessages;
}) {
  const bundle = useText();
  const rp = (text ?? bundle.research).replay;
  const tr = rp.trace;
  const locale = useLocale();
  const [table, setTable] = useState(false);
  const [hover, setHover] = useState<TracePoint | null>(null);
  const box: ChartBox = { ...BOX, t0: start, t1: end > start ? end : start + 1 };
  const points = trace?.trace.points ?? [];
  const bands = trace?.bands ?? [];
  const rects = bandRects(box, bands);
  const n2 = (v: number) => formatNumber(v, 2, locale);
  const plotR = box.width - box.pad.r;
  const ticks = Array.from({ length: Math.floor(box.t1 - box.t0) + 1 }, (_, i) => box.t0 + i).filter((t, i, all) => all.length <= 12 || i % 2 === 0);

  const move = (ev: MouseEvent<SVGSVGElement>) => {
    const r = ev.currentTarget.getBoundingClientRect();
    const vx = ((ev.clientX - r.left) / r.width) * box.width;
    const t = box.t0 + ((vx - box.pad.l) / (plotR - box.pad.l)) * (box.t1 - box.t0);
    setHover(nearestPoint(points, t));
  };
  const click = (ev: MouseEvent<SVGSVGElement>) => {
    if (!onSeek) return;
    const r = ev.currentTarget.getBoundingClientRect();
    const vx = ((ev.clientX - r.left) / r.width) * box.width;
    if (vx < box.pad.l || vx > plotR) return;
    onSeek(box.t0 + ((vx - box.pad.l) / (plotR - box.pad.l)) * (box.t1 - box.t0));
  };
  const download = () => {
    const blob = new Blob([traceCsv(clipId, `aoiDwell:${aoi}`, points)], { type: "text/csv;charset=utf-8" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = traceCsvName(clipId);
    document.body.appendChild(a);
    a.click();
    a.remove();
    setTimeout(() => URL.revokeObjectURL(url), 0);
  };
  const bandText = (low: number | null, high: number | null) =>
    low !== null && high !== null ? tr.band(n2(low), n2(high)) : low !== null ? tr.bandOpen(n2(low), true) : high !== null ? tr.bandOpen(n2(high), false) : null;

  return (
    <figure className={s.figure}>
      <figcaption>{tr.title(aoi)}</figcaption>
      <div className={s.chartBar}>
        <button type="button" aria-pressed={table} onClick={() => setTable((v) => !v)}>
          {table ? rp.chart : rp.table}
        </button>
        <button type="button" onClick={download} disabled={!points.length}>{rp.downloadCsv}</button>
        {trace ? <span className="muted">{tr.window(n2(trace.trace.windowSec), n2(trace.trace.stepSec))}</span> : null}
      </div>
      {table ? (
        <table className="data" aria-label={tr.title(aoi)} data-testid="replay-trace-table">
          <thead>
            <tr>
              <th scope="col">{tr.cols.tSec}</th>
              <th scope="col">{tr.cols.value}</th>
              <th scope="col">{tr.cols.validRatio}</th>
              <th scope="col">{tr.cols.nFrames}</th>
            </tr>
          </thead>
          <tbody>
            {points.map((p) => (
              <tr key={p.tSec}>
                <td>{n2(p.tSec)}</td>
                <td>{p.value === null ? tr.noValue : n2(p.value)}</td>
                <td>{n2(p.validRatio)}</td>
                <td>{p.nFrames}</td>
              </tr>
            ))}
          </tbody>
        </table>
      ) : (
        <svg
          className={s.chart}
          viewBox={`0 0 ${box.width} ${box.height}`}
          role="img"
          aria-label={tr.alt(aoi, points.length)}
          onMouseMove={move}
          onMouseLeave={() => setHover(null)}
          onClick={click}
          data-testid="replay-trace"
        >
          <g data-testid="replay-bands">
            {rects.map((b) => (
              <g key={b.key}>
                <rect className={s.band} x={b.x} y={b.y} width={b.w} height={b.h}>
                  <title>{bandText(b.band.low, b.band.high) ?? rp.expected}</title>
                </rect>
                {b.meanY !== null ? <line className={s.mean} x1={b.x} x2={b.x + b.w} y1={b.meanY} y2={b.meanY} /> : null}
              </g>
            ))}
          </g>
          {bands.flatMap((b) =>
            b.thresholds
              .filter((th) => th.value >= 0 && th.value <= 1)
              .map((th) => (
                <g key={`${b.segmentId}-${th.labelKey}`}>
                  <line className={s.threshold} x1={xOf(box, Math.max(b.startSec, box.t0))} x2={xOf(box, Math.min(b.endSec, box.t1))} y1={yOf(box, th.value)} y2={yOf(box, th.value)} />
                  <text className={s.thresholdLabel} x={plotR + 4} y={yOf(box, th.value) + 3}>
                    {tr.threshold(bandLabel(locale, th.labelKey), n2(th.value))}
                  </text>
                </g>
              )),
          )}
          <line className={s.axis} x1={box.pad.l} y1={box.height - box.pad.b} x2={plotR} y2={box.height - box.pad.b} />
          <line className={s.axis} x1={box.pad.l} y1={box.pad.t} x2={box.pad.l} y2={box.height - box.pad.b} />
          {[0, 0.5, 1].map((v) => (
            <text key={v} x={box.pad.l - 6} y={yOf(box, v) + 3} textAnchor="end">{n2(v)}</text>
          ))}
          {ticks.map((t) => (
            <text key={t} x={xOf(box, t)} y={box.height - box.pad.b + 12} textAnchor="middle">{formatNumber(t, 0, locale)}</text>
          ))}
          <text x={(box.pad.l + plotR) / 2} y={box.height - 4} textAnchor="middle">{tr.axisX}</text>
          <text x={10} y={box.pad.t + 2} transform={`rotate(-90 10 ${box.pad.t + 2})`} textAnchor="end">{tr.axisY(aoi)}</text>
          <path className={s.line} d={tracePath(box, points)} data-testid="replay-trace-line" />
          <line className={s.playhead} x1={xOf(box, tSec)} x2={xOf(box, tSec)} y1={box.pad.t} y2={box.height - box.pad.b} />
          {hover && hover.value !== null ? <circle className={s.hoverDot} cx={xOf(box, hover.tSec)} cy={yOf(box, hover.value)} r={4} /> : null}
        </svg>
      )}
      <p className="muted" style={{ margin: 0 }} aria-live="polite">
        {hover ? tr.point(n2(hover.tSec), hover.value === null ? tr.noValue : n2(hover.value), hover.nFrames) : null}
      </p>
      <ul className={s.legend}>
        <li><span className={`${s.swatch} ${s.bg_band}`} aria-hidden="true" />{rp.expected}</li>
        <li><span className={`${s.swatch} ${s.bg_line}`} aria-hidden="true" />{rp.recorded}</li>
        <li>{tr.mean}</li>
        {bands.map((b) => {
          const t = bandText(b.low, b.high);
          return t ? <li key={`${b.segmentId}-legend`}>{tr.segmentBand(b.segmentId, t)}</li> : null;
        })}
      </ul>
      {needAge ? <p className="muted" style={{ margin: 0 }}>{rp.needAge}</p> : !bands.length ? <p className="muted" style={{ margin: 0 }}>{tr.noBands}</p> : null}
    </figure>
  );
}
