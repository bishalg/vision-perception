/**
 * EMR-lite record (REC-1..10): participants create and search, encounters, attachments and follow-ups. Wave 2 step 0
 * registers this module in `types.ts`, `index.ts`, `live.ts` and `mock.ts` so WP-C.18 fills it (zod schemas for the
 * `api-v1.json` `$defs`, the `EmrApi` methods and their live implementation) without editing those shared files.
 */
import type { LiveRequest } from "./clinical";

/** Filled by WP-C.18. */
// eslint-disable-next-line @typescript-eslint/no-empty-object-type
export interface EmrApi {}

export function createLiveEmr(request: LiveRequest, buildQuery: (q: object) => string): EmrApi {
  void request;
  void buildQuery;
  return {};
}
