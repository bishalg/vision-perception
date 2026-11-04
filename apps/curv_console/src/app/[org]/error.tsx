"use client";

import Link from "next/link";
import { useParams } from "next/navigation";
import { useText } from "@/lib/i18n/provider";

/** Slug of the 403 a user past their two-step sign-in deadline gets (AUTH-13). */
const ENROLMENT_REQUIRED = "totp-enrolment-required";

/**
 * Errors from server components reach the browser stripped to a digest in production, so the ApiError (and its slug)
 * can only be read in development. The org layout already redirects such users, so this is the fallback for
 * the cases where it did not (for example a deadline that passed after the layout rendered).
 */
export default function OrgError({ error, reset }: { error: Error & { slug?: string | null }; reset: () => void }) {
  const { org } = useParams<{ org: string }>();
  const t = useText().app.errors;
  if (error.slug === ENROLMENT_REQUIRED || error.message.includes(ENROLMENT_REQUIRED)) {
    return (
      <div className="panel" role="alert">
        <h1>{t.title}</h1>
        <p>{t.enrolmentRequired}</p>
        <Link href={`/${encodeURIComponent(org)}/settings/security?required=1`}>{t.securityLink}</Link>
      </div>
    );
  }
  return (
    <div className="panel" role="alert">
      <h1>{t.title}</h1>
      <button type="button" onClick={reset}>
        {t.retry}
      </button>
    </div>
  );
}
