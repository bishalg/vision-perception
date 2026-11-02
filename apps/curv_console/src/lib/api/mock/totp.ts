/**
 * TOTP enrolment for mock mode (WP-C.15). The mock client is one process-wide instance (it does not know which
 * user is signed in), so this is one shared enrolment state. It accepts `MOCK_TOTP_CODE` as the current code
 * instead of computing RFC 6238 codes: the real seed and code checks live in the Rust backend.
 */
import { ApiError, PROBLEM_BASE } from "../types";
import type { TotpApi } from "../totp";

export interface MockTotpContext {
  password: string;
  code: string;
  now: () => number;
  /** Start state (default false). */
  enrolled?: boolean;
}

/** A fixed synthetic seed (RFC 4648 base32, 32 characters): mock only, never a real account's secret. */
export const MOCK_TOTP_SECRET = "JBSWY3DPEHPK3PXPJBSWY3DPEHPK3PXP";
const PENDING_MS = 10 * 60_000;

function problem(status: number, slug: string, title: string, detail: string): ApiError {
  return new ApiError(status, { type: `${PROBLEM_BASE}${slug}`, title, status, detail });
}

/** The enrolment API plus the state `GET /users/me` reports (`totp_enrolled`, `totp_pending`). */
export type MockTotp = TotpApi & { status(): { totp_enrolled: boolean; totp_pending: boolean } };

export function createMockTotp(ctx: MockTotpContext): MockTotp {
  let enrolled = ctx.enrolled ?? false;
  let pendingUntil: number | null = null;

  const checkPassword = (password: string) => {
    if (password !== ctx.password) throw problem(401, "invalid-credentials", "Unauthorized", "the password is not correct");
  };

  return {
    status: () => ({ totp_enrolled: enrolled, totp_pending: pendingUntil !== null && ctx.now() <= pendingUntil }),
    async enrolTotp(password) {
      checkPassword(password);
      if (enrolled) throw problem(409, "totp-already-enrolled", "Conflict", "two-step sign-in is already on");
      pendingUntil = ctx.now() + PENDING_MS;
      const label = encodeURIComponent("Curv Console:demo@example.org");
      return {
        secret: MOCK_TOTP_SECRET,
        otpauth_url: `otpauth://totp/${label}?secret=${MOCK_TOTP_SECRET}&issuer=Curv%20Console&algorithm=SHA1&digits=6&period=30`,
        expires_at: new Date(pendingUntil).toISOString(),
      };
    },
    async confirmTotp(totpCode) {
      if (enrolled) throw problem(409, "totp-already-enrolled", "Conflict", "two-step sign-in is already on");
      if (pendingUntil === null || ctx.now() > pendingUntil) {
        pendingUntil = null;
        throw problem(410, "totp-enrolment-expired", "Gone", "no pending enrolment");
      }
      if (totpCode !== ctx.code) throw problem(401, "totp-invalid", "Unauthorized", "the authenticator code is not correct");
      enrolled = true;
      pendingUntil = null;
      return { totp_enrolled: true };
    },
    async disableTotp(password, totpCode) {
      checkPassword(password);
      if (!enrolled) throw problem(404, "totp-not-enrolled", "Not found", "two-step sign-in is not on");
      if (!totpCode) throw problem(401, "totp-required", "Unauthorized", "an authenticator code is required");
      if (totpCode !== ctx.code) throw problem(401, "totp-invalid", "Unauthorized", "the authenticator code is not correct");
      enrolled = false;
      return { totp_enrolled: false };
    },
  };
}
