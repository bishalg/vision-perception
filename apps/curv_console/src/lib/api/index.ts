/** Single entry point for API access from server code. Selects mock or live by env. */
import { createLiveClient } from "./live";
import { createMockClient } from "./mock";
import type { AuthApi, CurvApi } from "./types";

export * from "./types";
export * from "./clinical";
export * from "./admin";
export * from "./exports";
export * from "./catalog";
export * from "./config";
export * from "./privacy";
export * from "./participant";
export * from "./totp";
export * from "./emr";
export * from "./share";
export * from "./research";

export type ApiMode = "mock" | "live";

export function apiMode(env: Record<string, string | undefined> = process.env): ApiMode {
  return env.CURV_API_MODE === "live" ? "live" : "mock";
}

/**
 * The mock keeps one state for the process. It lives on `globalThis`, not in a module variable: `next dev` loads route
 * handlers (`app/api/...`) in another module graph than the pages and server actions, and a module-level singleton
 * would give each graph its own mock (an attachment created by a server action unknown to the upload route, a share
 * token issued by the passcode step unknown to the download route).
 */
const MOCK_KEY = Symbol.for("curv-console.mock-client");

function mockClient(): ReturnType<typeof createMockClient> {
  const g = globalThis as unknown as Record<symbol, ReturnType<typeof createMockClient> | undefined>;
  return (g[MOCK_KEY] ??= createMockClient());
}

export function getApi(accessToken?: string, env: Record<string, string | undefined> = process.env): CurvApi & AuthApi {
  // The token only picks which mock user the shared mock answers for (WP-C.17).
  if (apiMode(env) === "mock") return mockClient().forToken(accessToken);
  const baseUrl = env.CURV_API_BASE_URL;
  if (!baseUrl) throw new Error("CURV_API_BASE_URL is required when CURV_API_MODE=live");
  return createLiveClient({ baseUrl, accessToken });
}
