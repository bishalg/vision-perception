import { DOMAIN, dt, type DomainKey, type DomainLocale } from "@/messages/domain";

/** The five caregiver-text cases of `asd_core::flag::assess` (I18N-4); the catalogue keys are `flag_assessment.<case>.{headline,detail}`. */
export const FLAG_CASES = ["no_scored_clips", "unreliable", "matches", "differs", "some_differ"] as const;
export type FlagCase = (typeof FLAG_CASES)[number];

/**
 * The case whose English headline equals `headline`, or null for text from another version. A catalogue lookup only:
 * a Rust test in `asd-contract` asserts that the catalogue `en` equals the constants the devices write, so the sent
 * English and `DOMAIN.en` cannot drift. (The asd-wasm `flagCase` export answers the same question; it is async to
 * load, so the synchronous render path reads the catalogue directly.)
 */
export function flagCase(headline: string): FlagCase | null {
  return FLAG_CASES.find((c) => DOMAIN.en[`flag_assessment.${c}.headline` as DomainKey] === headline) ?? null;
}

/** The headline and detail in `locale`; an unknown headline keeps the text the device sent (never hides it). */
export function flagAssessmentText(
  locale: DomainLocale,
  sent: { headline: string; detail?: string | null },
): { headline: string; detail: string | null } {
  const c = flagCase(sent.headline);
  if (!c) return { headline: sent.headline, detail: sent.detail ?? null };
  return {
    headline: dt(locale, `flag_assessment.${c}.headline` as DomainKey),
    detail: dt(locale, `flag_assessment.${c}.detail` as DomainKey),
  };
}
