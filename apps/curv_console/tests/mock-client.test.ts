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
