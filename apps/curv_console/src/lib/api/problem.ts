import { DOMAIN, dt, type DomainKey } from "@/messages/domain";
import type { Locale } from "@/lib/i18n/types";
import type { Problem } from "./types";

/** What `problemText` needs from a bundle (`messagesFor(locale)` and `useText()` both satisfy it). */
export interface HasLocale {
  locale: Locale;
}

export interface ProblemText {
  /** The catalogue sentence for the slug in the user's locale, else the server `title`. */
  text: string;
  /** The server `detail` (English until WP-L.4) when it adds information; shown as a secondary line. */
  detail: string | null;
}

const PROBLEM_PREFIX = "https://schemas.curv.org/asd/problems/";

/** The problem slug of an RFC 7807 `type` URI, or null. */
export function problemSlug(problem: Pick<Problem, "type">): string | null {
  return problem.type.startsWith(PROBLEM_PREFIX) ? problem.type.slice(PROBLEM_PREFIX.length) : null;
}

/** I18N-19: `problem.<slug>` from the domain catalogue when known, else the server title; detail kept separate. */
export function problemText(t: HasLocale, problem: Pick<Problem, "type" | "title" | "detail">): ProblemText {
  const slug = problemSlug(problem);
  const key = slug === null ? null : `problem.${slug}`;
  const known = key !== null && key in DOMAIN[t.locale];
  const text = known ? dt(t.locale, key as DomainKey) : problem.title;
  const detail = problem.detail?.trim() ? problem.detail.trim() : null;
  return { text, detail: detail && detail !== text ? detail : null };
}
