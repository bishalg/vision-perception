import "server-only";
import { headers } from "next/headers";
import { notFound, redirect } from "next/navigation";
import { ApiError } from "@/lib/api";
import { enforcementRedirect } from "@/lib/auth/guard";
import { orgFromPath } from "@/lib/orgs";

/**
 * Map API errors to Next navigation outcomes: 404 -> not-found, 401 -> login, 403 `totp-enrolment-required` -> the
 * security page, anything else bubbles to error.tsx.
 */
export async function loadOrNotFound<T>(load: () => Promise<T>): Promise<T> {
  try {
    return await load();
  } catch (err) {
    if (err instanceof ApiError) {
      // A 403 with this slug is not "missing" (AUTH-13). A page renders in parallel with the org layout, so its own
      // load can fail before the layout's redirect lands: send the user to the security page from here too, and
      // fall back to error.tsx when the org cannot be read from the path.
      if (err.slug === "totp-enrolment-required") {
        const org = orgFromPath((await headers()).get("x-pathname") ?? "");
        const to = org ? enforcementRedirect({ totp_enforcement: "required" }, null, org) : null;
        if (to) redirect(to);
        throw err;
      }
      if (err.status === 404 || err.status === 403) notFound();
      if (err.status === 401) redirect("/login");
    }
    throw err;
  }
}

/**
 * Wall-clock time of this server render. Server components render once per request, so reading the clock is
 * safe here; it is passed to client components so relative times match between server and client.
 */
export function requestTime(): number {
  return Date.now();
}
