"use client";

import { useText } from "@/lib/i18n/provider";

/** WP-C.17 (AUTH-13): shown on the security page when the role requires two-step sign-in and the deadline passed. */
export function RequiredPanel() {
  const t = useText().security.security;
  return (
    <section className="panel" role="alert" aria-labelledby="totp-required-title" data-testid="totp-required">
      <h2 id="totp-required-title">{t.required.title}</h2>
      <p>{t.required.intro}</p>
    </section>
  );
}
