import { describe, expect, it, vi } from "vitest";
import { buildSessionQuery, createLiveClient } from "@/lib/api/live";
import { ApiError, PROBLEM_BASE } from "@/lib/api";

const json = (body: unknown, status = 200) =>
  new Response(JSON.stringify(body), { status, headers: { "Content-Type": "application/json" } });

const session = {
  id: "s1", participant_id: "7f1c1c1e-0000-4000-8000-000000000001", participant_token: "child-001", started_at: "2026-11-01T10:00:00Z", duration_sec: 200,
  quality_verdict: "optimal", attention_flag: "green", composite_score: 50, composite_confidence: 0.9,
  bundle_version: "2026.11.0",
};

describe("live client", () => {
  it("sends bearer token and builds the sessions URL with filters and cursor", async () => {
    const f = vi.fn().mockResolvedValue(json({ items: [session], next_cursor: "abc" }));
    const api = createLiveClient({ baseUrl: "https://api.test/", accessToken: "tok", fetchImpl: f as unknown as typeof fetch });
    const page = await api.listSessions("org 1", { flag: "red", cursor: "c1", limit: 10 });
    const [url, init] = f.mock.calls[0]!;
    expect(url).toBe("https://api.test/v1/orgs/org%201/sessions?flag=red&cursor=c1&limit=10");
    expect(init.headers.Authorization).toBe("Bearer tok");
    expect(page.next_cursor).toBe("abc");
  });

  it("omits the auth header on login and posts credentials", async () => {
    const f = vi.fn().mockResolvedValue(
      json({ access_token: "a", refresh_token: "r", expires_in: 900, user: { id: "u", email: "e@x.org", role: "admin", org_id: "o" } }),
    );
    const api = createLiveClient({ baseUrl: "https://api.test", accessToken: "stale", fetchImpl: f as unknown as typeof fetch });
    const pair = await api.login("e@x.org", "pw");
    const [url, init] = f.mock.calls[0]!;
    expect(url).toBe("https://api.test/v1/auth/login");
    expect(init.method).toBe("POST");
    expect(init.headers.Authorization).toBeUndefined();
    expect(JSON.parse(init.body)).toEqual({ email: "e@x.org", password: "pw" });
    expect(pair.user.org_id).toBe("o");
  });

  it("raises ApiError carrying the problem+json body", async () => {
    const f = vi.fn().mockResolvedValue(json({ type: `${PROBLEM_BASE}forbidden`, title: "Forbidden", status: 403, detail: "no" }, 403));
    const api = createLiveClient({ baseUrl: "https://api.test", fetchImpl: f as unknown as typeof fetch });
    await expect(api.getSession("x")).rejects.toMatchObject({ status: 403, problem: { detail: "no" }, slug: "forbidden" });
    await expect(api.getSession("x")).rejects.toBeInstanceOf(ApiError);
  });

  it("rejects responses that violate the contract", async () => {
    const f = vi.fn().mockResolvedValue(json({ items: [{ id: "s1" }] }));
    const api = createLiveClient({ baseUrl: "https://api.test", fetchImpl: f as unknown as typeof fetch });
    await expect(api.listSessions("o")).rejects.toThrow();
  });

  it("keeps validation-failed invalid_params and tolerates a non-problem error body", async () => {
    const f = vi.fn()
      .mockResolvedValueOnce(json({
        type: `${PROBLEM_BASE}validation-failed`, title: "Validation failed", status: 400,
        invalid_params: [{ name: "cursor", reason: "invalid" }],
      }, 400))
      .mockResolvedValueOnce(new Response("<html>bad gateway</html>", { status: 502 }));
    const api = createLiveClient({ baseUrl: "https://api.test", fetchImpl: f as unknown as typeof fetch });
    await expect(api.listSessions("o", { cursor: "x" })).rejects.toMatchObject({
      status: 400, slug: "validation-failed", problem: { invalid_params: [{ name: "cursor", reason: "invalid" }] },
    });
    await expect(api.getSession("x")).rejects.toMatchObject({ status: 502, problem: null, slug: null });
  });

  it("refresh and logout send {refresh_token}; logout accepts 204 with no body and sends the bearer", async () => {
    const f = vi.fn()
      .mockResolvedValueOnce(json({ access_token: "a2", refresh_token: "r2", expires_in: 900, user: { id: "u", email: "e@x.org", role: "clinician", org_id: "o" } }))
      .mockResolvedValueOnce(new Response(null, { status: 204 }));
    const api = createLiveClient({ baseUrl: "https://api.test", accessToken: "tok", fetchImpl: f as unknown as typeof fetch });
    expect((await api.refresh("r1")).refresh_token).toBe("r2");
    expect(JSON.parse(f.mock.calls[0]![1].body)).toEqual({ refresh_token: "r1" });
    expect(f.mock.calls[0]![1].headers.Authorization).toBeUndefined();
    await expect(api.logout("r2")).resolves.toBeUndefined();
    expect(f.mock.calls[1]![0]).toBe("https://api.test/v1/auth/logout");
    expect(JSON.parse(f.mock.calls[1]![1].body)).toEqual({ refresh_token: "r2" });
  });

  it("uses org-scoped paths for org resources and unscoped paths for sessions", async () => {
    const f = vi.fn().mockImplementation(async () => json({}));
    const api = createLiveClient({ baseUrl: "https://api.test", accessToken: "t", fetchImpl: f as unknown as typeof fetch });
    await api.getOrg("o1").catch(() => {});
    await api.getMetrics("o1").catch(() => {});
    await api.getSession("s 1").catch(() => {});
    await api.getSessionClips("s 1").catch(() => {});
    expect(f.mock.calls.map((c) => c[0])).toEqual([
      "https://api.test/v1/orgs/o1", "https://api.test/v1/orgs/o1/metrics",
      "https://api.test/v1/sessions/s%201", "https://api.test/v1/sessions/s%201/clips",
    ]);
  });

  it("unwraps the clips envelope and defaults rows to []", async () => {
    const clip = { clip_id: "c", order: 0, status: null };
    const f = vi.fn().mockResolvedValue(json({ items: [clip] }));
    const api = createLiveClient({ baseUrl: "https://api.test", fetchImpl: f as unknown as typeof fetch });
    const clips = await api.getSessionClips("s1");
    expect(clips).toHaveLength(1);
    expect(clips[0]!.rows).toEqual([]);
  });

  it("skips empty query values", () => {
    expect(buildSessionQuery({ flag: undefined, quality: "" })).toBe("");
  });
});
