/** Single entry point for API access from server code. Selects mock or live by env. */
import { createLiveClient } from "./live";
import { createMockClient } from "./mock";
import type { AuthApi, CurvApi } from "./types";

export * from "./types";
export * from "./clinical";

export type ApiMode = "mock" | "live";

export function apiMode(env: Record<string, string | undefined> = process.env): ApiMode {
  return env.CURV_API_MODE === "live" ? "live" : "mock";
}

let mockSingleton: (CurvApi & AuthApi) | undefined;

export function getApi(accessToken?: string, env: Record<string, string | undefined> = process.env): CurvApi & AuthApi {
  if (apiMode(env) === "mock") return (mockSingleton ??= createMockClient());
  const baseUrl = env.CURV_API_BASE_URL;
  if (!baseUrl) throw new Error("CURV_API_BASE_URL is required when CURV_API_MODE=live");
  return createLiveClient({ baseUrl, accessToken });
}
