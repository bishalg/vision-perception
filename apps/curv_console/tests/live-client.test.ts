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

  const reviewItem = {
    id: "r1", org_id: "o1", session_id: "S1", reason: "flag_red", status: "open",
    created_at: "2026-10-30T09:00:00.000Z", assigned_to: null, decision: null, decided_by: null, decided_at: null,
  };
  const device = {
    id: "d1", org_id: "o1", platform: "ios", app_version: "1.4.0", enrolled_at: "2026-10-30T09:00:00.000Z",
    label: "Room 2", revoked_at: null, last_seen_at: null, config_bundle_version: null,
  };

  it("lists the review queue with status/reason/cursor and patches an item with a JSON body", async () => {
    const f = vi.fn()
      .mockResolvedValueOnce(json({ items: [reviewItem], next_cursor: null, total: 1 }))
      .mockResolvedValueOnce(json({ ...reviewItem, status: "done" }));
    const api = createLiveClient({ baseUrl: "https://api.test", accessToken: "t", fetchImpl: f as unknown as typeof fetch });
    const page = await api.listReview("o1", { status: "open", reason: "flag_red", cursor: "c", limit: 25 });
    expect(page.total).toBe(1);
    expect(f.mock.calls[0]![0]).toBe("https://api.test/v1/orgs/o1/review?status=open&reason=flag_red&cursor=c&limit=25");
    const patch = { status: "done" as const, decision: { action: "confirm" }, labels: [{ key: "review", value: "confirmed" }] };
    const updated = await api.updateReview("r 1", patch);
    expect(updated.status).toBe("done");
    const [url, init] = f.mock.calls[1]!;
    expect(url).toBe("https://api.test/v1/review/r%201");
    expect(init.method).toBe("PATCH");
    expect(init.headers.Authorization).toBe("Bearer t");
    expect(JSON.parse(init.body)).toEqual(patch);
  });

  it("lists devices, revokes with POST /devices/{id}/revoke and creates an enrol code", async () => {
    const f = vi.fn()
      .mockResolvedValueOnce(json({ items: [device], next_cursor: "n" }))
      .mockResolvedValueOnce(json({ ...device, revoked_at: "2026-10-31T09:00:00.000Z" }))
      .mockResolvedValueOnce(json({ id: "e1", code: "K7Q4-9XMP", max_uses: 2, expires_at: "2026-11-06T12:00:00.000Z" }, 201));
    const api = createLiveClient({ baseUrl: "https://api.test", accessToken: "t", fetchImpl: f as unknown as typeof fetch });
    expect((await api.listDevices("o1", { cursor: "c" })).next_cursor).toBe("n");
    expect(f.mock.calls[0]![0]).toBe("https://api.test/v1/orgs/o1/devices?cursor=c");
    expect((await api.revokeDevice("d1")).revoked_at).not.toBeNull();
    expect(f.mock.calls[1]![0]).toBe("https://api.test/v1/devices/d1/revoke");
    expect(f.mock.calls[1]![1].method).toBe("POST");
    const code = await api.createEnrolCode("o1", { label: "Room 2", max_uses: 2 });
    expect(code.code).toBe("K7Q4-9XMP");
    expect(f.mock.calls[2]![0]).toBe("https://api.test/v1/orgs/o1/devices/enrol-code");
    expect(JSON.parse(f.mock.calls[2]![1].body)).toEqual({ label: "Room 2", max_uses: 2 });
  });

  it("rejects a review item whose reason is outside the contract", async () => {
    const f = vi.fn().mockResolvedValue(json({ items: [{ ...reviewItem, reason: "quality_low" }] }));
    const api = createLiveClient({ baseUrl: "https://api.test", fetchImpl: f as unknown as typeof fetch });
    await expect(api.listReview("o1")).rejects.toThrow();
  });
});
