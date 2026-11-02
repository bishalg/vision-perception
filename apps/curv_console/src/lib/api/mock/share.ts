/** Mock share links (wave 2 step 0 stub; WP-C.19 fills it). Synthetic data only. */
import type { ShareApi } from "../share";

export interface MockShareContext {
  orgId: string;
  userId: string;
  /** Epoch milliseconds (`MOCK_FIXED_NOW`). */
  now: number;
}

export function createMockShare(ctx: MockShareContext): ShareApi {
  void ctx;
  return {};
}
