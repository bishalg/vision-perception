"use client";

import { useEffect, useRef, useState } from "react";

export type JobPollState = "idle" | "polling" | "final" | "error" | "timeout";

export interface JobPollOptions<T> {
  /** Polling runs only while true; a falsy value (no job yet, or the job is already final) keeps the hook idle. */
  enabled: boolean;
  /** Identifies the job; a new key restarts the poll and forgets the previous answer. */
  jobKey: string;
  /** One poll. Throwing (or rejecting) stops polling and is reported as `error`. */
  fetch: () => Promise<T>;
  /** True for a state that never changes again (succeeded, failed, cancelled, done). Polling stops there. */
  isFinal: (value: T) => boolean;
  intervalMs?: number;
  maxPolls?: number;
  /** Called with every answer, final or not. */
  onValue?: (value: T) => void;
}

export interface JobPoll<T> {
  /** The latest answer for the current `jobKey`, or null before the first one. */
  value: T | null;
  /** What `fetch` threw, when `state` is `error`. */
  error: unknown;
  state: JobPollState;
}

interface Snapshot<T> {
  key: string;
  value: T | null;
  error: unknown;
  state: JobPollState;
}

const DEFAULT_INTERVAL_MS = 1000;
const DEFAULT_MAX_POLLS = 120;

/**
 * Polls a background job until it reaches a final state (WP-X.7; shared by the retention sweep and the analysis runs).
 * The first poll comes one interval after the job starts. Polling stops on a final answer, on an error, after
 * `maxPolls`, when `enabled` turns false, and on unmount: no timer or request outlives the component's use of it.
 */
export function useJobPoll<T>(options: JobPollOptions<T>): JobPoll<T> {
  const { enabled, jobKey, intervalMs = DEFAULT_INTERVAL_MS, maxPolls = DEFAULT_MAX_POLLS } = options;
  const [snap, setSnap] = useState<Snapshot<T> | null>(null);
  // The latest callbacks, without restarting the poll each render.
  const latest = useRef(options);
  useEffect(() => {
    latest.current = options;
  });

  useEffect(() => {
    if (!enabled) return;
    let alive = true;
    let timer: ReturnType<typeof setTimeout> | undefined;
    let polls = 0;
    const publish = (next: Omit<Snapshot<T>, "key">) => {
      if (alive) setSnap({ key: jobKey, ...next });
    };
    const tick = async () => {
      polls += 1;
      let value: T;
      try {
        value = await latest.current.fetch();
      } catch (error) {
        publish({ value: null, error, state: "error" });
        return;
      }
      if (!alive) return;
      latest.current.onValue?.(value);
      if (latest.current.isFinal(value)) return publish({ value, error: null, state: "final" });
      if (polls >= maxPolls) return publish({ value, error: null, state: "timeout" });
      publish({ value, error: null, state: "polling" });
      timer = setTimeout(tick, intervalMs);
    };
    timer = setTimeout(tick, intervalMs);
    return () => {
      alive = false;
      if (timer) clearTimeout(timer);
    };
  }, [enabled, jobKey, intervalMs, maxPolls]);

  if (!enabled && snap?.key !== jobKey) return { value: null, error: null, state: "idle" };
  if (!snap || snap.key !== jobKey) return { value: null, error: null, state: enabled ? "polling" : "idle" };
  return { value: snap.value, error: snap.error, state: snap.state };
}
