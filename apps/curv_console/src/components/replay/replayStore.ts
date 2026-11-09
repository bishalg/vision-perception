"use client";

/**
 * The replay playhead shared between the session's Replay tab and its Quality tab (ADR-0015 V-4): one small external
 * store per page load, keyed by session, read with `useSyncExternalStore`. It holds where the scrubber is, nothing
 * else; every value in it came from the frames or the Rust core.
 */
import { useSyncExternalStore } from "react";

export interface Playhead {
  sessionId: string;
  clipId: string;
  clipTitle: string;
  /** Clip media time, seconds. */
  clipSec: number;
  /** Session time of the frame nearest the playhead (`video_time_sec`), seconds; null before any frame. */
  sessionSec: number | null;
}

let current: Playhead | null = null;
const listeners = new Set<() => void>();

export function setPlayhead(next: Playhead | null): void {
  if (
    current &&
    next &&
    current.sessionId === next.sessionId &&
    current.clipId === next.clipId &&
    current.clipSec === next.clipSec &&
    current.sessionSec === next.sessionSec
  ) {
    return;
  }
  current = next;
  for (const l of listeners) l();
}

export function getPlayhead(): Playhead | null {
  return current;
}

function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

/** The playhead of `sessionId`, or null when the replay has not placed one for it. */
export function usePlayhead(sessionId: string): Playhead | null {
  const p = useSyncExternalStore(subscribe, getPlayhead, () => null);
  return p && p.sessionId === sessionId ? p : null;
}
