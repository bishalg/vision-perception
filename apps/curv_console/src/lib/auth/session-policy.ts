/** HIP-2 automatic logoff policy (WP-C.21): pure, so proxy.ts and the tests share one definition. */

/** Idle timeout in minutes (owner decision O-10). */
export const IDLE_TIMEOUT_MIN = 30;
/** Absolute session length in hours (owner decision O-10). */
export const SESSION_MAX_HOURS = 12;

export const IDLE_TIMEOUT_MS = IDLE_TIMEOUT_MIN * 60 * 1000;
export const SESSION_MAX_MS = SESSION_MAX_HOURS * 60 * 60 * 1000;
/** Tolerated clock skew between the stamp and `now`; a timestamp further in the future is treated as forged. */
const SKEW_MS = 60 * 1000;

export type SessionVerdict = "ok" | "idle" | "expired";

/** Parse a timestamp cookie (epoch milliseconds). Anything else is `undefined`. */
export function parseStamp(value: string | undefined): number | undefined {
  if (!value || !/^\d{1,15}$/.test(value)) return undefined;
  return Number(value);
}

/**
 * Decide whether a session that still holds auth cookies may continue. Epoch milliseconds throughout.
 * A missing, malformed or future-dated stamp is `expired` (fail closed: a session from before this policy
 * has no stamps and must sign in again). The absolute limit wins over the idle one.
 */
export function evaluate(lastSeen: number | undefined, loginAt: number | undefined, now: number): SessionVerdict {
  if (lastSeen === undefined || loginAt === undefined) return "expired";
  if (!Number.isFinite(lastSeen) || !Number.isFinite(loginAt)) return "expired";
  if (loginAt > now + SKEW_MS || lastSeen > now + SKEW_MS) return "expired";
  if (now - loginAt >= SESSION_MAX_MS) return "expired";
  if (now - lastSeen >= IDLE_TIMEOUT_MS) return "idle";
  return "ok";
}
