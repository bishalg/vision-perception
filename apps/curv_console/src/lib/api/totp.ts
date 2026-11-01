/**
 * Self-service TOTP enrolment (WP-C.15, AUTH-6..8, ADR-0009): zod schemas for the `api-v1.json` DTOs, the `TotpApi`
 * interface and its live HTTP implementation. The backend generates, stores and checks the seed and the codes; the
 * console only relays the form fields and shows the pending seed once.
 */
import { z } from "@/lib/zod";
import type { LiveRequest } from "./clinical";

/** `POST /v1/users/me/totp/enrol` body. */
export const totpEnrolRequestSchema = z.object({ password: z.string() });
export type TotpEnrolRequest = z.infer<typeof totpEnrolRequestSchema>;

/** The pending seed: shown once (QR of `otpauth_url` and the text secret), never stored by the console. */
export const totpEnrolResponseSchema = z.object({
  secret: z.string().min(1),
  otpauth_url: z.string().min(1),
  /** RFC 3339; after it `confirm` answers `410 totp-enrolment-expired`. */
  expires_at: z.string(),
});
export type TotpEnrolResponse = z.infer<typeof totpEnrolResponseSchema>;

/** `POST /v1/users/me/totp/confirm` body. */
export const totpConfirmRequestSchema = z.object({ totp_code: z.string() });
export type TotpConfirmRequest = z.infer<typeof totpConfirmRequestSchema>;

/** `DELETE /v1/users/me/totp` body. */
export const totpDisableRequestSchema = z.object({
  password: z.string(),
  totp_code: z.string().optional(),
});
export type TotpDisableRequest = z.infer<typeof totpDisableRequestSchema>;

/** Result of confirm and disable. */
export const totpStatusSchema = z.object({ totp_enrolled: z.boolean() });
export type TotpStatus = z.infer<typeof totpStatusSchema>;

export interface TotpApi {
  /** Starts (or restarts) enrolment for the signed-in user; the answer is the only time the seed is in clear. */
  enrolTotp(password: string): Promise<TotpEnrolResponse>;
  confirmTotp(totpCode: string): Promise<TotpStatus>;
  disableTotp(password: string, totpCode?: string): Promise<TotpStatus>;
}

export function createLiveTotp(request: LiveRequest): TotpApi {
  return {
    enrolTotp: (password) =>
      request("/users/me/totp/enrol", totpEnrolResponseSchema, { method: "POST", body: { password } }),
    confirmTotp: (totp_code) =>
      request("/users/me/totp/confirm", totpStatusSchema, { method: "POST", body: { totp_code } }),
    disableTotp: (password, totpCode) =>
      request("/users/me/totp", totpStatusSchema, {
        method: "DELETE",
        body: totpCode ? { password, totp_code: totpCode } : { password },
      }),
  };
}
