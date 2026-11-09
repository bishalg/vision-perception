"use client";

import { useCallback, useEffect, useMemo, useState } from "react";
import { browserCore } from "@/lib/asd/browser";
import type { AsdCore } from "@/lib/asd/core";
import {
  catalogClip,
  replayEvents,
  replayRibbon,
  replayTrace,
  targetAoiOf,
  type GazeFrame,
  type ReplayEvents,
  type ReplayRibbon,
  type ReplayTrace,
} from "@/lib/asd/replay";
import { useText } from "@/lib/i18n/provider";
import { ClipList, type ClipEntry } from "./ClipList";
import { clipSamples, clipSpan, sampleIndexAt } from "./drawModel";
import { HeatmapPair } from "./HeatmapPair";
import { ReplayCanvas } from "./ReplayCanvas";
import { setPlayhead } from "./replayStore";
import { TimelineStrip } from "./TimelineStrip";
import { TraceChart } from "./TraceChart";
import s from "./replay.module.css";

/** V-2: the trace is the share of gaze on the target AOI in a sliding 1 s window, every 0.25 s. */
export const TRACE_WINDOW_SEC = 1;
export const TRACE_STEP_SEC = 0.25;

export interface SessionReplayProps {
  sessionId: string;
  /** `raw_payload.frames` of the stored record; null when the record has none. */
  frames: GazeFrame[] | null;
  /** The record's `ageMonths`, the default of the age field. */
  ageMonths: number | null;
  /** Effective configuration bodies (GOV-6); null when the caller cannot read them. */
  catalog: unknown;
  baselines: unknown;
  profile: unknown;
  /** The stored scoring status of each clip (worst row), by clip id. */
  clipStatuses: Record<string, string | null>;
  core?: AsdCore;
}

/**
 * The session detail's Replay tab (ADR-0015 V-1..V-3): canvas, clip list with recorded versus expected, timeline,
 * expected-versus-actual trace and heatmaps. Every number comes from the Rust core through WASM.
 */
export function SessionReplay(props: SessionReplayProps) {
  const { sessionId, frames, catalog, baselines, profile, clipStatuses } = props;
  const rp = useText().research.replay;
  const core = useMemo(() => props.core ?? browserCore(), [props.core]);
  const [events, setEvents] = useState<ReplayEvents | null>(null);
  const [problem, setProblem] = useState<string | null>(null);
  const [clipId, setClipId] = useState<string>("");
  // The playhead belongs to a clip: another clip starts at its first frame (no effect needed).
  const [pos, setPos] = useState<{ clip: string; t: number } | null>(null);
  const [age, setAge] = useState<number | null>(props.ageMonths);
  const [ribbon, setRibbon] = useState<ReplayRibbon | null>(null);
  const [trace, setTrace] = useState<ReplayTrace | null>(null);
  const [entries, setEntries] = useState<ClipEntry[]>([]);

  const fail = useCallback(
    (e: { kind: string; message: string }) => setProblem(e.kind === "unavailable" ? rp.coreUnavailable : rp.failed(e.message)),
    [rp],
  );

  // Events of every clip (the profile's fixation knobs).
  useEffect(() => {
    if (!frames) return;
    let live = true;
    void replayEvents(core, frames, profile ?? {}).then((r) => {
      if (!live) return;
      if (!r.ok) return fail(r.error);
      setEvents(r.result);
      setClipId((cur) => cur || r.result.clips[0]?.stimulusId || "");
    });
    return () => {
      live = false;
    };
  }, [core, frames, profile, fail]);

  const clipIds = useMemo(() => events?.clips.map((c) => c.stimulusId) ?? [], [events]);
  const clip = useMemo(() => (clipId ? catalogClip(catalog, clipId) : null), [catalog, clipId]);
  const aoi = useMemo(() => targetAoiOf(baselines, clip), [baselines, clip]);
  const samples = useMemo(() => (frames && clipId ? clipSamples(frames, clipId) : []), [frames, clipId]);
  const span = useMemo(() => clipSpan(samples), [samples]);
  const clipEvents = useMemo(() => events?.clips.find((c) => c.stimulusId === clipId)?.events ?? [], [events, clipId]);

  // Ribbon and trace of the selected clip.
  useEffect(() => {
    if (!frames || !clip) return;
    let live = true;
    void (async () => {
      const rb = await replayRibbon(core, frames, clip, profile ?? undefined);
      if (!live) return;
      if (!rb.ok) return fail(rb.error);
      setRibbon(rb.result);
      if (!aoi || !baselines) return setTrace(null);
      const tr = await replayTrace(core, frames, clip, baselines, profile ?? {}, {
        metric: { kind: "aoiDwell", aoi },
        windowSec: TRACE_WINDOW_SEC,
        stepSec: TRACE_STEP_SEC,
        ageMonths: age,
      });
      if (!live) return;
      if (!tr.ok) return fail(tr.error);
      setTrace(tr.result);
    })();
    return () => {
      live = false;
    };
  }, [core, frames, clip, aoi, baselines, profile, age, fail]);

  // Clip cards: the whole-clip share on the target AOI is the core's trace over one window that covers the clip.
  useEffect(() => {
    if (!frames || !clipIds.length) return;
    let live = true;
    void (async () => {
      const out: ClipEntry[] = [];
      for (const id of clipIds) {
        const c = catalogClip(catalog, id);
        const target = targetAoiOf(baselines, c);
        let recorded: number | null = null;
        let band = null;
        if (c && target && baselines) {
          const whole = c.durationSec + 1;
          const tr = await replayTrace(core, frames, c, baselines, profile ?? {}, {
            metric: { kind: "aoiDwell", aoi: target },
            windowSec: 2 * whole,
            stepSec: whole,
            ageMonths: age,
          });
          if (tr.ok) {
            recorded = tr.result.trace.points[0]?.value ?? null;
            band = tr.result.bands[0] ?? null;
          }
        }
        out.push({
          id,
          title: c?.title ?? id,
          durationSec: c?.durationSec ?? null,
          recorded,
          band,
          status: clipStatuses[id] ?? null,
          aoi: target,
        });
      }
      if (live) setEntries(out);
    })();
    return () => {
      live = false;
    };
  }, [core, frames, clipIds, catalog, baselines, profile, age, clipStatuses]);

  const tSec = pos && pos.clip === clipId ? pos.t : span.start;
  const shownRibbon = ribbon && clip && ribbon.stimulusId === clip.id ? ribbon : null;
  const shownTrace = trace && clip && trace.stimulusId === clip.id ? trace : null;
  // Share the playhead with the Quality tab (V-4).
  const title = clip?.title ?? clipId;
  useEffect(() => {
    if (!clipId) return;
    const i = sampleIndexAt(samples, tSec);
    setPlayhead({ sessionId, clipId, clipTitle: title, clipSec: tSec, sessionSec: samples[i]?.sessionSec ?? null });
  }, [sessionId, clipId, title, tSec, samples]);

  const seek = useCallback((t: number) => setPos({ clip: clipId, t }), [clipId]);

  if (!frames) return <p className="muted">{rp.noFrames}</p>;
  return (
    <section className={s.stack} aria-label={rp.title} data-testid="replay">
      <p className="muted" style={{ margin: 0 }}>{rp.intro}</p>
      {problem ? <p role="alert" className="muted">{problem}</p> : null}
      {events?.warnings.map((w) => <p key={w} className="muted" style={{ margin: 0 }}>{rp.warning(w)}</p>)}
      <div className={s.layout}>
        <div className={s.stack}>
          {clipId ? (
            <ReplayCanvas
              clipId={clipId}
              clipTitle={title}
              areas={clip?.areas ?? []}
              events={clipEvents}
              samples={samples}
              segments={shownRibbon?.segments ?? []}
              tSec={tSec}
              onSeek={seek}
              clips={entries.map((e) => ({ id: e.id, title: e.title }))}
              onClip={setClipId}
            />
          ) : null}
          <p className="muted" style={{ margin: 0 }}>{rp.noMedia}</p>
          {clipId && !clip ? <p className="muted" style={{ margin: 0 }}>{rp.noClip(clipId)}</p> : null}
        </div>
        <div className={s.stack}>
          <label>
            {rp.age}{" "}
            <input
              type="number"
              min={0}
              max={240}
              step={1}
              value={age ?? ""}
              onChange={(ev) => {
                const v = ev.currentTarget.value;
                setAge(v === "" ? null : Math.max(0, Math.round(Number(v))));
              }}
            />
          </label>
          <ClipList clips={entries} selected={clipId} onSelect={setClipId} />
        </div>
      </div>
      {clipId ? (
        <TimelineStrip
          clipTitle={title}
          start={span.start}
          end={span.end}
          segments={shownRibbon?.segments ?? []}
          events={clipEvents}
          areas={clip?.areas ?? []}
          tSec={tSec}
          onSeek={seek}
        />
      ) : null}
      {clipId && aoi ? (
        <TraceChart clipId={clipId} aoi={aoi} trace={shownTrace} start={span.start} end={span.end} tSec={tSec} onSeek={seek} needAge={age === null} />
      ) : null}
      {clipId ? <HeatmapPair core={core} frames={frames} clipId={clipId} areas={clip?.areas ?? []} cohortGrids={null} /> : null}
    </section>
  );
}
