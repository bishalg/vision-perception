import { describe, expect, it } from "vitest";
import {
  IDLE_TIMEOUT_MIN,
  IDLE_TIMEOUT_MS,
  SESSION_MAX_HOURS,
  SESSION_MAX_MS,
  evaluate,
  parseStamp,
} from "@/lib/auth/session-policy";

const NOW = 1_800_000_000_000;
const MIN = 60_000;
const HOUR = 60 * MIN;

describe("session policy constants", () => {
  it("are 30 minutes and 12 hours", () => {
    expect(IDLE_TIMEOUT_MIN).toBe(30);
    expect(SESSION_MAX_HOURS).toBe(12);
    expect(IDLE_TIMEOUT_MS).toBe(30 * MIN);
    expect(SESSION_MAX_MS).toBe(12 * HOUR);
  });
});

describe("evaluate", () => {
  it.each([
    ["fresh", NOW - MIN, NOW - HOUR, "ok"],
    ["just under idle", NOW - 29 * MIN - 59_000, NOW - HOUR, "ok"],
    ["exactly idle", NOW - 30 * MIN, NOW - HOUR, "idle"],
    ["idle 31 min", NOW - 31 * MIN, NOW - HOUR, "idle"],
    ["just under absolute", NOW - MIN, NOW - 12 * HOUR + 1000, "ok"],
    ["exactly absolute", NOW - MIN, NOW - 12 * HOUR, "expired"],
    ["active but 13 h old", NOW - MIN, NOW - 13 * HOUR, "expired"],
    ["idle and expired: expired wins", NOW - 2 * HOUR, NOW - 13 * HOUR, "expired"],
    ["missing last_seen", undefined, NOW - HOUR, "expired"],
    ["missing login_at", NOW - MIN, undefined, "expired"],
    ["both missing", undefined, undefined, "expired"],
    ["NaN stamp", Number.NaN, NOW - HOUR, "expired"],
    ["login_at in the future", NOW, NOW + 2 * MIN, "expired"],
    ["last_seen in the future", NOW + 2 * MIN, NOW - HOUR, "expired"],
    ["small clock skew tolerated", NOW + 10_000, NOW - HOUR, "ok"],
  ] as const)("%s -> %s", (_n, lastSeen, loginAt, want) => {
    expect(evaluate(lastSeen, loginAt, NOW)).toBe(want);
  });
});

describe("parseStamp", () => {
  it("accepts digits only", () => {
    expect(parseStamp("1800000000000")).toBe(1_800_000_000_000);
    for (const bad of [undefined, "", "abc", "-5", "1.5", "1e9", "99999999999999999"]) {
      expect(parseStamp(bad)).toBeUndefined();
    }
  });
});
