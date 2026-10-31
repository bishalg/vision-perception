import { describe, expect, it } from "vitest";
import { MOCK_ORG_ID, MOCK_PASSWORD, createMockClient } from "@/lib/api/mock";
import { apiMode, getApi } from "@/lib/api";

describe("mock client", () => {
  const api = createMockClient();

  it("logs in only with the demo password", async () => {
    await expect(api.login("a@b.c", "nope")).rejects.toMatchObject({ status: 401 });
    expect((await api.login("a@b.c", MOCK_PASSWORD)).user.org_id).toBe(MOCK_ORG_ID);
  });

  it("paginates with cursors without overlap and ends with a null cursor", async () => {
    const seen = new Set<string>();
    let cursor: string | undefined;
    let pages = 0;
    do {
      const page = await api.listSessions(MOCK_ORG_ID, { limit: 25, cursor });
      for (const s of page.items) {
        expect(seen.has(s.id)).toBe(false);
        seen.add(s.id);
      }
      cursor = page.next_cursor ?? undefined;
      pages++;
    } while (cursor);
    expect(seen.size).toBe(64);
    expect(pages).toBe(3);
  });

  it("uses opaque base64url keyset cursors and rejects a tampered one with problem+json 400", async () => {
    const page = await api.listSessions(MOCK_ORG_ID, { limit: 5 });
    expect(page.next_cursor).toMatch(/^[A-Za-z0-9_-]+$/);
    await expect(api.listSessions(MOCK_ORG_ID, { cursor: "bogus" })).rejects.toMatchObject({ status: 400, slug: "validation-failed" });
  });

  it("filters by flag and date range", async () => {
    const red = await api.listSessions(MOCK_ORG_ID, { flag: "red", limit: 100 });
    expect(red.items.length).toBeGreaterThan(0);
    expect(red.items.every((s) => s.attention_flag === "red")).toBe(true);
    const none = await api.listSessions(MOCK_ORG_ID, { from: "2030-01-01" });
    expect(none.items).toEqual([]);
  });

  it("metrics are consistent with the sessions list", async () => {
    const m = await api.getMetrics(MOCK_ORG_ID);
    expect(Object.values(m.flag_distribution).reduce((a, b) => a + b, 0)).toBe(m.sessions_30d);
    expect(m.sessions_7d).toBeLessThanOrEqual(m.sessions_30d);
  });

  it("returns 404 for unknown org and session", async () => {
    await expect(api.getOrg("other")).rejects.toMatchObject({ status: 404 });
    await expect(api.getSession("nope")).rejects.toMatchObject({ status: 404 });
  });

  it("selects mock/live from env", () => {
    expect(apiMode({})).toBe("mock");
    expect(apiMode({ CURV_API_MODE: "live" })).toBe("live");
    expect(() => getApi("t", { CURV_API_MODE: "live" })).toThrow(/CURV_API_BASE_URL/);
  });
});

describe("mock client: review queue", () => {
  it("lists open items newest first with every contract reason, filters, and paginates by cursor", async () => {
    const api = createMockClient();
    const open = await api.listReview(MOCK_ORG_ID, { status: "open", limit: 100 });
    expect(open.items.length).toBeGreaterThan(5);
    expect(open.items.every((i) => i.status === "open")).toBe(true);
    const created = open.items.map((i) => i.created_at);
    expect([...created].sort().reverse()).toEqual(created);
    const all = await api.listReview(MOCK_ORG_ID, { limit: 100 });
    expect(new Set(all.items.map((i) => i.reason))).toEqual(new Set(["low_quality", "flag_red", "random_audit", "norm_candidate", "manual", "low_tracking_quality"]));
    const red = await api.listReview(MOCK_ORG_ID, { reason: "flag_red", limit: 100 });
    expect(red.items.every((i) => i.reason === "flag_red")).toBe(true);
    const first = await api.listReview(MOCK_ORG_ID, { status: "open", limit: 3 });
    const second = await api.listReview(MOCK_ORG_ID, { status: "open", limit: 3, cursor: first.next_cursor! });
    expect(second.items[0]!.id).not.toBe(first.items[2]!.id);
    expect(first.total).toBe(open.items.length);
  });

  it("PATCH closes an item: decided_by/at are set, the queue and metrics shrink, the session is reviewed", async () => {
    const api = createMockClient();
    const before = await api.getMetrics(MOCK_ORG_ID);
    const [item] = (await api.listReview(MOCK_ORG_ID, { status: "open", limit: 1 })).items;
    const updated = await api.updateReview(item!.id, {
      status: "done", decision: { action: "confirm" }, labels: [{ key: "review", value: "confirmed" }],
    });
    expect(updated).toMatchObject({ status: "done", decision: { action: "confirm" } });
    expect(updated.decided_at).toBeTruthy();
    expect(updated.decided_by).toBeTruthy();
    expect((await api.getMetrics(MOCK_ORG_ID)).review_queue_size).toBe(before.review_queue_size - 1);
    expect((await api.getSession(item!.session_id)).reviewed).toBe(true);
    const reopened = await api.updateReview(item!.id, { status: "open" });
    expect(reopened.decided_at).toBeNull();
  });

  it("PATCH rejects an unknown item (404) and an out-of-contract status (400)", async () => {
    const api = createMockClient();
    await expect(api.updateReview("nope", { status: "done" })).rejects.toMatchObject({ status: 404 });
    const [item] = (await api.listReview(MOCK_ORG_ID, { limit: 1 })).items;
    await expect(api.updateReview(item!.id, { status: "closed" as never })).rejects.toMatchObject({ status: 400, slug: "validation-failed" });
  });

  it("review_queue_size counts open and in_review items, as the backend does", async () => {
    const api = createMockClient();
    const open = (await api.listReview(MOCK_ORG_ID, { status: "open", limit: 100 })).total!;
    const inReview = (await api.listReview(MOCK_ORG_ID, { status: "in_review", limit: 100 })).total!;
    expect((await api.getMetrics(MOCK_ORG_ID)).review_queue_size).toBe(open + inReview);
  });
});

describe("mock client: devices", () => {
  it("lists devices with one revoked; revoke is idempotent and 404s for an unknown device", async () => {
    const api = createMockClient();
    const { items } = await api.listDevices(MOCK_ORG_ID);
    expect(items.length).toBeGreaterThanOrEqual(4);
    expect(items.some((d) => d.revoked_at)).toBe(true);
    const live = items.find((d) => !d.revoked_at)!;
    const revoked = await api.revokeDevice(live.id);
    expect(revoked.revoked_at).toBeTruthy();
    expect((await api.revokeDevice(live.id)).revoked_at).toBe(revoked.revoked_at);
    await expect(api.revokeDevice("nope")).rejects.toMatchObject({ status: 404 });
  });

  it("creates enrol codes with backend defaults and validates limits like the backend", async () => {
    const api = createMockClient();
    const code = await api.createEnrolCode(MOCK_ORG_ID);
    expect(code.code).toMatch(/^[A-Z0-9]{4}-[A-Z0-9]{4}$/);
    expect(code.max_uses).toBe(1);
    const days = (Date.parse(code.expires_at) - Date.now()) / 86_400_000;
    expect(days).toBeGreaterThan(6.9);
    expect(days).toBeLessThan(7.1);
    expect((await api.createEnrolCode(MOCK_ORG_ID, { max_uses: 5, expires_in_days: 30 })).max_uses).toBe(5);
    await expect(api.createEnrolCode(MOCK_ORG_ID, { max_uses: 0 })).rejects.toMatchObject({ status: 400 });
    await expect(api.createEnrolCode(MOCK_ORG_ID, { expires_in_days: 91 })).rejects.toMatchObject({ status: 400 });
    await expect(api.createEnrolCode("other", {})).rejects.toMatchObject({ status: 404 });
  });

  it("signs in as the role named by the email's local part (for role-gated UI), else admin; superadmin has no org", async () => {
    const api = createMockClient();
    expect((await api.login("viewer@example.org", MOCK_PASSWORD)).user.role).toBe("viewer");
    expect((await api.login("demo@example.org", MOCK_PASSWORD)).user.role).toBe("admin");
    expect((await api.login("superadmin@example.org", MOCK_PASSWORD)).user).toMatchObject({ role: "superadmin", org_id: null });
    expect((await api.login("owner@example.org", MOCK_PASSWORD)).user).toMatchObject({ role: "owner", org_id: MOCK_ORG_ID });
  });
});
