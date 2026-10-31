/** Live HTTP implementation of the typed API. This is the only module that calls `fetch` against the backend. */
import type { ZodType } from "zod";
import {
  ApiError,
  clipListSchema,
  deviceSchema,
  devicePageSchema,
  enrolCodeResponseSchema,
  metricsSchema,
  orgSchema,
  reviewItemSchema,
  reviewPageSchema,
  sessionDetailSchema,
  sessionPageSchema,
  tokenPairSchema,
  type AuthApi,
  type CurvApi,
  problemSchema,
  type Problem,
  type SessionListQuery,
} from "./types";

export interface LiveOptions {
  /** Backend origin, e.g. https://api.example.org (no trailing slash, no /v1). */
  baseUrl: string;
  accessToken?: string;
  fetchImpl?: typeof fetch;
}

/** Query string for any list endpoint; undefined and empty values are skipped. */
export function buildQuery(query: object = {}): string {
  const params = new URLSearchParams();
  for (const [key, value] of Object.entries(query)) {
    if (value === undefined || value === "") continue;
    params.set(key, String(value));
  }
  const text = params.toString();
  return text ? `?${text}` : "";
}

export function buildSessionQuery(query: SessionListQuery = {}): string {
  return buildQuery(query);
}

async function toApiError(res: Response): Promise<ApiError> {
  // Backend errors are RFC 7807 `application/problem+json`; anything else (proxy 502, HTML) carries no problem.
  let problem: Problem | null = null;
  try {
    const parsed = problemSchema.safeParse(await res.json());
    if (parsed.success) problem = parsed.data;
  } catch {
    problem = null;
  }
  return new ApiError(res.status, problem);
}

export function createLiveClient(opts: LiveOptions): CurvApi & AuthApi {
  const base = opts.baseUrl.replace(/\/+$/, "");
  const doFetch = opts.fetchImpl ?? fetch;

  async function request<T>(
    path: string,
    schema: ZodType<T> | null,
    init: { method?: string; body?: unknown; auth?: boolean } = {},
  ): Promise<T> {
    const headers: Record<string, string> = { Accept: "application/json" };
    if (init.body !== undefined) headers["Content-Type"] = "application/json";
    if (init.auth !== false && opts.accessToken) headers.Authorization = `Bearer ${opts.accessToken}`;
    const res = await doFetch(`${base}/v1${path}`, {
      method: init.method ?? "GET",
      headers,
      body: init.body === undefined ? undefined : JSON.stringify(init.body),
      cache: "no-store",
    });
    if (!res.ok) throw await toApiError(res);
    if (schema === null) return undefined as T;
    return schema.parse(await res.json());
  }

  const enc = encodeURIComponent;
  return {
    login: (email, password) =>
      request("/auth/login", tokenPairSchema, { method: "POST", body: { email, password }, auth: false }),
    refresh: (refresh_token) =>
      request("/auth/refresh", tokenPairSchema, { method: "POST", body: { refresh_token }, auth: false }),
    logout: (refresh_token) => request("/auth/logout", null, { method: "POST", body: { refresh_token } }),
    getOrg: (orgId) => request(`/orgs/${enc(orgId)}`, orgSchema),
    getMetrics: (orgId) => request(`/orgs/${enc(orgId)}/metrics`, metricsSchema),
    listSessions: (orgId, query) =>
      request(`/orgs/${enc(orgId)}/sessions${buildSessionQuery(query)}`, sessionPageSchema),
    getSession: (id) => request(`/sessions/${enc(id)}`, sessionDetailSchema),
    getSessionClips: async (id) => (await request(`/sessions/${enc(id)}/clips`, clipListSchema)).items,
    listReview: (orgId, query) => request(`/orgs/${enc(orgId)}/review${buildQuery(query)}`, reviewPageSchema),
    updateReview: (id, patch) => request(`/review/${enc(id)}`, reviewItemSchema, { method: "PATCH", body: patch }),
    listDevices: (orgId, query) => request(`/orgs/${enc(orgId)}/devices${buildQuery(query)}`, devicePageSchema),
    revokeDevice: (id) => request(`/devices/${enc(id)}/revoke`, deviceSchema, { method: "POST" }),
    createEnrolCode: (orgId, body) =>
      request(`/orgs/${enc(orgId)}/devices/enrol-code`, enrolCodeResponseSchema, { method: "POST", body: body ?? {} }),
  };
}
