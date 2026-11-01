/** User-visible strings for the account security page (WP-C.15 TOTP enrolment). en only; merged by import. */

const errors: Record<string, string> = {
  "invalid-credentials": "The password is not correct.",
  "totp-invalid": "That code is not correct or was already used. Wait for the next code and try again.",
  "totp-required": "Enter a code from your authenticator app.",
  "totp-already-enrolled": "Two-step sign-in is already on. Turn it off first to set up a new authenticator.",
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
    text: "Your role can see or change sensitive data. Turn on two-step sign-in to protect your account.",
    link: "Set it up",
  },
  status: {
    pending: "A setup was started and is waiting for a code. Get a new key below to start again.",
  },
  title: "Account security",
  intro:
    "Two-step sign-in asks for a code from an authenticator app after your password. Owners, admins and clinicians should turn it on.",
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
