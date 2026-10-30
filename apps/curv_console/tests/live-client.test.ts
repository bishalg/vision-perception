import { describe, expect, it, vi } from "vitest";
import { buildSessionQuery, createLiveClient } from "@/lib/api/live";
import { ApiError } from "@/lib/api";

const json = (body: unknown, status = 200) =>
  new Response(JSON.stringify(body), { status, headers: { "Content-Type": "application/json" } });

const session = {
  id: "s1", participant_id: "child-001", started_at: "2026-11-01T10:00:00Z", duration_sec: 200,
  quality_verdict: "good", attention_flag: "green", composite_score: 50, composite_confidence: 0.9,
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
    const f = vi.fn().mockResolvedValue(json({ title: "Forbidden", status: 403, detail: "no" }, 403));
    const api = createLiveClient({ baseUrl: "https://api.test", fetchImpl: f as unknown as typeof fetch });
    await expect(api.getSession("x")).rejects.toMatchObject({ status: 403, problem: { detail: "no" } });
    await expect(api.getSession("x")).rejects.toBeInstanceOf(ApiError);
  });

  it("rejects responses that violate the contract", async () => {
    const f = vi.fn().mockResolvedValue(json({ items: [{ id: "s1" }] }));
    const api = createLiveClient({ baseUrl: "https://api.test", fetchImpl: f as unknown as typeof fetch });
    await expect(api.listSessions("o")).rejects.toThrow();
  });

  it("unwraps the clips envelope", async () => {
    const clip = { clip_id: "c", order: 0, status: "onTarget" };
    const f = vi.fn().mockResolvedValue(json({ items: [clip] }));
    const api = createLiveClient({ baseUrl: "https://api.test", fetchImpl: f as unknown as typeof fetch });
    expect(await api.getSessionClips("s1")).toHaveLength(1);
  });

  it("skips empty query values", () => {
    expect(buildSessionQuery({ flag: undefined, quality: "" })).toBe("");
  });
});
