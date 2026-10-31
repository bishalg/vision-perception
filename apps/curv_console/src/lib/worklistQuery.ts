/** URL state for the worklist: enum values and an opaque cursor only; anything else is dropped (UI-4). */
import { attentionFlagSchema } from "@/lib/api";
import { clinicalStateSchema, worklistAssignedSchema, type WorklistQuery } from "@/lib/api/clinical";
import { parseCursor } from "@/lib/listQuery";

type Raw = Record<string, string | string[] | undefined>;

export function parseWorklistQuery(raw: Raw): WorklistQuery {
  const one = (k: string) => {
    const v = raw[k];
    return Array.isArray(v) ? v[0] : v;
  };
  const state = clinicalStateSchema.safeParse(one("state"));
  const flag = attentionFlagSchema.safeParse(one("flag"));
  const assigned = worklistAssignedSchema.safeParse(one("assigned"));
  const cursor = parseCursor(raw);
  return {
    ...(state.success ? { state: state.data } : {}),
    ...(flag.success ? { flag: flag.data } : {}),
    ...(assigned.success ? { assigned: assigned.data } : {}),
    ...(cursor ? { cursor } : {}),
  };
}
