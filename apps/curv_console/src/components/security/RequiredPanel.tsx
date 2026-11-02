import { securityText } from "@/messages/security";

/** WP-C.17 (AUTH-13): shown on the security page when the role requires two-step sign-in and the deadline passed. */
export function RequiredPanel() {
  return (
    <section className="panel" role="alert" aria-labelledby="totp-required-title" data-testid="totp-required">
      <h2 id="totp-required-title">{securityText.required.title}</h2>
      <p>{securityText.required.intro}</p>
    </section>
  );
}
