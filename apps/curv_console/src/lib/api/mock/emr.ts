/** Mock EMR-lite record (wave 2 step 0 stub; WP-C.18 fills it). Synthetic data only. */
import type { EmrApi } from "../emr";

export interface MockEmrContext {
  orgId: string;
  userId: string;
  /** Epoch milliseconds (`MOCK_FIXED_NOW`). */
  now: number;
}

export function createMockEmr(ctx: MockEmrContext): EmrApi {
  void ctx;
  return {};
}
