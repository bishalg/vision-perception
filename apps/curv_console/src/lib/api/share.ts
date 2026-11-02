/**
 * Share-a-case links (SHR-8, SHR-9): create, list and revoke share links of a case-package export, and the public
 * open/download of `/share/[id]`. Wave 2 step 0 registers this module in `types.ts`, `index.ts`, `live.ts` and
 * `mock.ts` so WP-C.19 fills it without editing those shared files.
 */
import type { LiveRequest } from "./clinical";

/** Filled by WP-C.19. */
// eslint-disable-next-line @typescript-eslint/no-empty-object-type
export interface ShareApi {}

export function createLiveShare(request: LiveRequest, buildQuery: (q: object) => string): ShareApi {
  void request;
  void buildQuery;
  return {};
}
