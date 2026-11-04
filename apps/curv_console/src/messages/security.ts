/** User-visible strings for the account security page (WP-C.15 TOTP enrolment). en only; merged by import. */
import type { Widen } from "@/lib/i18n/types";

const errors: Record<string, string> = {
  "invalid-credentials": "The password is not correct.",
  "totp-invalid": "That code is not correct or was already used. Wait for the next code and try again.",
  "totp-required": "Enter a code from your authenticator app.",
  "totp-already-enrolled": "Two-step sign-in is already on. Turn it off first to set up a new authenticator.",
  "totp-enrolment-required":
    "Your role requires two-step sign-in. Set it up on the account security page to continue.",
  "totp-enrolment-expired": "The setup expired. Start again to get a new key.",
  "totp-not-enrolled": "Two-step sign-in is not on for your account.",
  "clinical-crypto-unavailable": "The server cannot store authenticator keys right now. Contact your administrator.",
  "rate-limited": "Too many attempts. Wait a minute and try again.",
  "validation-failed": "Some fields were rejected.",
  unavailable: "Could not reach the server. Try again shortly.",
  fallback: "The request failed. Try again.",
};

export function securityErrorMessage(slug: string | null, status: number): string {
  if (slug && errors[slug]) return errors[slug];
  if (status === 429) return errors["rate-limited"]!;
  if (status === 0) return errors.unavailable!;
  return errors.fallback!;
}

export const securityText = {
  nav: "Account security",
  nudge: {
    text: "Your role requires two-step sign-in. Turn on two-step sign-in to protect your account.",
    deadline: (date: string) => `Set it up by ${date}.`,
    link: "Set it up",
  },
  required: {
    title: "Your role requires two-step sign-in",
    intro: "The time to set it up has passed. Set up two-step sign-in below to continue; other pages stay locked until you do.",
  },
  status: {
    pending: "A setup was started and is waiting for a code. Get a new key below to start again.",
  },
  title: "Account security",
  intro:
    "Two-step sign-in asks for a code from an authenticator app after your password. Some roles must turn it on.",
  enrol: {
    title: "Set up two-step sign-in",
    intro: "Confirm your password to get a new key for your authenticator app.",
    password: "Current password",
    start: "Get a key",
    qr: "QR code for your authenticator app",
    scan: "Scan the QR code with your authenticator app, or enter the key by hand.",
    secret: "Key",
    once: "This key is shown only once. Do not share it or store it with your password.",
    expires: (when: string) => `Confirm before ${when}.`,
    code: "Code from your authenticator app",
    confirm: "Confirm",
    done: "Two-step sign-in is on. You will be asked for a code each time you sign in.",
  },
  disable: {
    title: "Turn off two-step sign-in",
    intro: "Confirm your password and a current code from your authenticator app.",
    password: "Current password",
    code: "Code from your authenticator app",
    submit: "Turn off",
    done: "Two-step sign-in is off.",
  },
} as const;

/** HIP-2 automatic logoff banners on /login (`?reason=idle|expired`). */
export const logoffText = {
  idle: "You were signed out because the console was idle for 30 minutes. Sign in again to continue.",
  expired: "Your session reached its 12-hour limit. Sign in again to continue.",
} as const;

/** WP-L.1.0 (I18N-15): both locales of this file. `ne` is typed by the `en` tree; the wave package replaces the copy. */
const en = { security: securityText, logoff: logoffText };
const ne: Widen<typeof en> = en; // ne: TODO
export const securityMessages = { en, ne } as const;
