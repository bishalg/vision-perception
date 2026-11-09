"use client";

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { AsdCore } from "@/lib/asd/core";
import { defaultScoringProfile } from "@/lib/asd/ops";
import { catalogClip, framesOfRecord, replayEvents, replayRibbon, type GazeFrame, type ReplayEvents, type ReplayRibbon } from "@/lib/asd/replay";
import { debounce } from "@/lib/research/preview";
import type { ProfileDoc } from "@/lib/research/registry";
import type { ResearchMessages } from "@/messages/research";
import { clipSamples, clipSpan } from "./drawModel";
import { ReplayCanvas } from "./ReplayCanvas";
import { TimelineStrip } from "./TimelineStrip";
import s from "./replay.module.css";

interface Side {
  events: ReplayEvents;
  ribbon: ReplayRibbon | null;
}

/**
 * The workbench tuning view (ADR-0015 V-6): one session's frames replayed with the published and with the draft
 * profile side by side, re-run through the Rust core 150 ms after the last knob change, so fixations and AOI hits can
 * be watched while a knob moves. The time and the clip are shared by both canvases.
 */
export function TuningReplay({
  core,
  sessionId,
  getRecord,
  catalog,
  publishedProfile,
  draft,
  text,
}: {
  core: AsdCore;
  sessionId: string;
  getRecord: (id: string) => Promise<Record<string, unknown> | null>;
  catalog: unknown;
  publishedProfile: ProfileDoc | null;
  draft: ProfileDoc;
  text: ResearchMessages;
}) {
  const rp = text.replay;
  const [frames, setFrames] = useState<GazeFrame[] | null | undefined>(undefined);
  const [published, setPublished] = useState<Side | null>(null);
  const [drafted, setDrafted] = useState<Side | null>(null);
  const [problem, setProblem] = useState<string | null>(null);
  const [clipId, setClipId] = useState("");
  const [tSec, setT] = useState(0);
  const getRef = useRef(getRecord);
  useEffect(() => {
    getRef.current = getRecord;
  });

  useEffect(() => {
    let live = true;
    void getRef.current(sessionId).then((rec) => {
      if (live) setFrames(framesOfRecord(rec));
    });
    return () => {
      live = false;
    };
  }, [sessionId]);

  const clip = useMemo(() => (clipId ? catalogClip(catalog, clipId) : null), [catalog, clipId]);

  const side = useCallback(
    async (profile: unknown): Promise<Side | string> => {
      if (!frames) return "";
      const ev = await replayEvents(core, frames, profile);
      if (!ev.ok) return ev.error.kind === "unavailable" ? rp.coreUnavailable : rp.failed(ev.error.message);
      const id = clipId || ev.result.clips[0]?.stimulusId || "";
      const c = catalogClip(catalog, id);
      const rb = c ? await replayRibbon(core, frames, c, profile) : null;
      return { events: ev.result, ribbon: rb?.ok ? rb.result : null };
    },
    [core, frames, catalog, clipId, rp],
  );

  // Published side: once per session, clip and published profile.
  useEffect(() => {
    let live = true;
    void (async () => {
      let pub: unknown = publishedProfile;
      if (!pub) {
        const d = await defaultScoringProfile(core);
        pub = d.ok ? d.result : {};
      }
      const r = await side(pub);
      if (!live) return;
      if (typeof r === "string") setProblem(r || null);
      else {
        setPublished(r);
        setClipId((cur) => cur || r.events.clips[0]?.stimulusId || "");
      }
    })();
    return () => {
      live = false;
    };
  }, [core, publishedProfile, side]);

  // Draft side: debounced 150 ms after the last change.
  const latest = useRef<() => Promise<void>>(async () => undefined);
  useEffect(() => {
    latest.current = async () => {
      const r = await side(draft);
      if (typeof r === "string") setProblem(r || null);
      else setDrafted(r);
    };
  });
  const draftKey = JSON.stringify(draft);
  useEffect(() => {
    const run = debounce(() => void latest.current());
    run();
    return () => run.cancel();
  }, [draftKey, frames, clipId]);

  const samples = useMemo(() => (frames && clipId ? clipSamples(frames, clipId) : []), [frames, clipId]);
  const span = useMemo(() => clipSpan(samples), [samples]);
  const seek = useCallback((t: number) => setT(t), []);
  const pickClip = (id: string) => {
    setClipId(id);
    setT(clipSpan(frames ? clipSamples(frames, id) : []).start);
  };

  if (frames === undefined) return null;
  if (frames === null) return <p className="muted">{rp.tuning.noFrames}</p>;
  const clips = (published?.events.clips ?? []).map((c) => ({ id: c.stimulusId, title: catalogClip(catalog, c.stimulusId)?.title ?? c.stimulusId }));
  const title = clip?.title ?? clipId;
  const pair = (which: Side | null, caption: string, controls: boolean) => (
    <div className={s.stack} data-testid="tuning-side">
      <ReplayCanvas
        clipId={clipId}
        clipTitle={title}
        areas={clip?.areas ?? []}
        events={which?.events.clips.find((c) => c.stimulusId === clipId)?.events ?? []}
        samples={samples}
        segments={which?.ribbon?.segments ?? []}
        tSec={tSec}
        onSeek={seek}
        clips={clips}
        onClip={pickClip}
        controls={controls}
        caption={caption}
        text={text}
      />
      <TimelineStrip
        clipTitle={title}
        start={span.start}
        end={span.end}
        segments={which?.ribbon?.segments ?? []}
        events={which?.events.clips.find((c) => c.stimulusId === clipId)?.events ?? []}
        areas={clip?.areas ?? []}
        tSec={tSec}
        onSeek={seek}
        text={text}
      />
    </div>
  );
  return (
    <section className={s.stack} aria-label={rp.tuning.toggle} data-testid="tuning-replay">
      <p className="muted" style={{ margin: 0 }}>{rp.tuning.intro}</p>
      {problem ? <p role="alert" className="muted">{problem}</p> : null}
      {clipId && !clip ? <p className="muted" style={{ margin: 0 }}>{rp.noClip(clipId)}</p> : null}
      {clipId ? (
        <div className={s.pair}>
          {pair(published, rp.tuning.published, true)}
          {pair(drafted, rp.tuning.draft, false)}
        </div>
      ) : null}
    </section>
  );
}
