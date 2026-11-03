/**
 * WP-X.9 (AGE-1): reads `summary.age_profile`, the block the Rust job writes (`asd_core::analysis::age_profile`). The
 * contract types it as arbitrary JSON, so the console parses it here and treats anything it cannot read as absent. No
 * number is computed: shares, percentiles and suppression come from the core.
 */
import { z } from "@/lib/zod";
import { distSchema } from "@/lib/api/research";

const optNum = z.number().nullable().optional();
const count = z.number().int().min(0);

const sideSchema = z.object({
  n: count.nullable().optional(),
  suppressed: z.boolean().optional(),
  dist: distSchema,
  range: z.object({ expected: optNum, acceptable_low: optNum, acceptable_high: optNum }).nullable().optional(),
  shares: z.object({ on_target: z.number(), monitor: z.number(), atypical: z.number() }).nullable().optional(),
  expected_percentile: optNum,
});
export type AgeProfileSide = z.infer<typeof sideSchema>;

const expectationSchema = z.object({
  stimulus_id: z.string(),
  segment_id: z.string(),
  biomarker: z.string(),
  target_aoi: z.string().nullable().optional(),
  metric_key: z.string(),
  direction: z.string(),
  published: sideSchema,
  draft: sideSchema,
});
export type AgeProfileExpectation = z.infer<typeof expectationSchema>;

const bandSchema = z.object({
  band: z.string(),
  min_months: z.number().int(),
  max_months: z.number().int(),
  n: count.nullable().optional(),
  suppressed: z.boolean().optional(),
  expectations: z.array(expectationSchema),
});
export type AgeProfileBand = z.infer<typeof bandSchema>;

export const ageProfileSchema = z.object({
  baselines: z.object({ published: z.string(), draft: z.string() }),
  min_n: count,
  /** `max(k, norms.minN)`: cells below it carry no number. */
  suppression_threshold: count.optional(),
  bands: z.array(bandSchema),
});
export type AgeProfile = z.infer<typeof ageProfileSchema>;

/** The age-profile block of a run summary, or `null` when absent or unreadable. */
export function parseAgeProfile(raw: unknown): AgeProfile | null {
  if (raw == null) return null;
  const parsed = ageProfileSchema.safeParse(raw);
  return parsed.success ? parsed.data : null;
}
