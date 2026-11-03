/** One readable line per filter of a cohort definition (COH-1); an empty definition reads "All consented sessions". */
import type { CohortDefinition } from "@/lib/api/research";
import { researchText, type ResearchMessages } from "@/messages/research";

export function describeCohort(def: CohortDefinition, text: ResearchMessages = researchText): string[] {
  const s = text.cohorts.summary;
  const lines: string[] = [];
  if (def.age_bands?.length) lines.push(s.ageBands(def.age_bands.join(", ")));
  if (def.from || def.to) lines.push(s.range(def.from ?? text.cohorts.any, def.to ?? text.cohorts.any));
  if (def.quality_verdicts?.length) lines.push(s.quality(def.quality_verdicts.join(", ")));
  if (def.tracking_verdicts?.length) lines.push(s.tracking(def.tracking_verdicts.join(", ")));
  if (def.min_valid_ratio != null) lines.push(s.minValid(String(def.min_valid_ratio)));
  if (def.require_frames) lines.push(s.framesOnly);
  if (def.include_synthetic) lines.push(s.synthetic);
  return lines.length ? lines : [s.all];
}
