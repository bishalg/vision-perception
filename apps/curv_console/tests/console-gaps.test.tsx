/** Console gaps (C.3, I.8): user read, audit filters, integration paging, exporter destination picker, delete. */
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { AuditFilters } from "@/components/AuditFilters";
import { ExportForm } from "@/components/exports/ExportForm";
import { DeleteIntegrationPanel } from "@/components/exports/IntegrationPanels";
import { createLiveClient } from "@/lib/api/live";
import { createMockClient, MOCK_ORG_ID } from "@/lib/api/mock";
import { auditFilterParams, parseAuditQuery } from "@/lib/listQuery";

const push = vi.fn();
vi.mock("next/navigation", () => ({ useRouter: () => ({ push, refresh: vi.fn() }) }));

const json = (body: unknown, status = 200) =>
  new Response(JSON.stringify(body), { status, headers: { "Content-Type": "application/json" } });

function live(response: Response) {
  const f = vi.fn().mockResolvedValue(response);
  return { f, api: createLiveClient({ baseUrl: "https://api.test", accessToken: "t", fetchImpl: f as unknown as typeof fetch }) };
}

describe("live wire format", () => {
  it("reads one user", async () => {
    const u = { id: "u/1", email: "a@b.org", role: "viewer", org_id: "o", status: "active" };
    const { f, api } = live(json(u));
    expect((await api.getUser("o 1", "u/1")).email).toBe("a@b.org");
    expect(f.mock.calls[0]![0]).toBe("https://api.test/v1/orgs/o%201/users/u%2F1");
    expect(f.mock.calls[0]![1].method).toBe("GET");
  });

  it("sends the audit filters and the cursor as query parameters", async () => {
    const { f, api } = live(json({ items: [] }));
    await api.listAudit("o", {
      actor_id: "a", action: "integration.", target_type: "integration", target_id: "t",
      from: "2026-10-01T00:00:00.000Z", to: "2026-10-31T23:59:59.999Z", cursor: "c", limit: 50,
    });
    const url = new URL(f.mock.calls[0]![0] as string);
    expect(url.pathname).toBe("/v1/orgs/o/audit");
    expect(Object.fromEntries(url.searchParams)).toEqual({
      actor_id: "a", action: "integration.", target_type: "integration", target_id: "t",
      from: "2026-10-01T00:00:00.000Z", to: "2026-10-31T23:59:59.999Z", cursor: "c", limit: "50",
    });
  });

  it("pages integrations, lists exporter destinations and deletes with DELETE", async () => {
    const a = live(json({ items: [], next_cursor: null, total: 0 }));
    await a.api.listIntegrations("o", { cursor: "c", limit: 20 });
    expect(a.f.mock.calls[0]![0]).toBe("https://api.test/v1/orgs/o/integrations?cursor=c&limit=20");
    const b = live(json({ items: [{ id: "d", name: "Hook", kind: "webhook", environment: "test", status: "active" }] }));
    expect((await b.api.listExportDestinations("o")).items[0]!.name).toBe("Hook");
    expect(b.f.mock.calls[0]![0]).toBe("https://api.test/v1/orgs/o/export-destinations");
    const c = live(new Response(null, { status: 204 }));
    await expect(c.api.deleteIntegration("d/1")).resolves.toBeUndefined();
    expect(c.f.mock.calls[0]![0]).toBe("https://api.test/v1/integrations/d%2F1");
    expect(c.f.mock.calls[0]![1].method).toBe("DELETE");
    const d = live(json({ type: "https://x/problems/conflict", title: "Conflict", status: 409 }, 409));
    await expect(d.api.deleteIntegration("d")).rejects.toMatchObject({ status: 409 });
  });

  it("rejects an exporter destination that carries an unknown kind", async () => {
    const { api } = live(json({ items: [{ id: "d", name: "x", kind: "ftp", environment: "test", status: "active" }] }));
    await expect(api.listExportDestinations("o")).rejects.toThrow();
  });
});

describe("mock client", () => {
  it("reads one user and 404s another org's or an unknown id", async () => {
    const api = createMockClient();
    const first = (await api.listUsers(MOCK_ORG_ID)).items[0]!;
    expect(await api.getUser(MOCK_ORG_ID, first.id)).toEqual(first);
    await expect(api.getUser(MOCK_ORG_ID, "nope")).rejects.toMatchObject({ status: 404 });
    await expect(api.getUser("other", first.id)).rejects.toMatchObject({ status: 404 });
  });

  it("filters the audit log by actor, action prefix, target and time, keeping pagination", async () => {
    const api = createMockClient();
    const all = (await api.listAudit(MOCK_ORG_ID, { limit: 100 })).items;
    const creates = (await api.listAudit(MOCK_ORG_ID, { action: "user.", limit: 100 })).items;
    expect(creates.length).toBeGreaterThan(0);
    expect(creates.every((e) => e.action.startsWith("user."))).toBe(true);
    expect(creates.length).toBeLessThan(all.length);
    const one = all.find((e) => e.target_id)!;
    const byTarget = (await api.listAudit(MOCK_ORG_ID, { target_type: one.target_type!, target_id: one.target_id! })).items;
    expect(byTarget.every((e) => e.target_id === one.target_id)).toBe(true);
    const actor = all.find((e) => e.actor_id)!.actor_id!;
    expect((await api.listAudit(MOCK_ORG_ID, { actor_id: actor, limit: 100 })).items.every((e) => e.actor_id === actor)).toBe(true);
    expect((await api.listAudit(MOCK_ORG_ID, { from: "2999-01-01T00:00:00Z" })).items).toEqual([]);
    expect((await api.listAudit(MOCK_ORG_ID, { to: "2000-01-01T00:00:00Z" })).items).toEqual([]);
    const seen = new Set<string>();
    let cursor: string | undefined;
    do {
      const page = await api.listAudit(MOCK_ORG_ID, { action: "review.", limit: 3, cursor });
      for (const e of page.items) {
        expect(e.action.startsWith("review.")).toBe(true);
        expect(seen.has(e.id)).toBe(false);
        seen.add(e.id);
      }
      cursor = page.next_cursor ?? undefined;
    } while (cursor);
    expect(seen.size).toBeGreaterThan(3);
    for (const q of [{ from: "yesterday" }, { from: "2026-02-01T00:00:00Z", to: "2026-01-01T00:00:00Z" }]) {
      await expect(api.listAudit(MOCK_ORG_ID, q)).rejects.toMatchObject({ status: 400 });
    }
  });

  it("pages integrations newest first with a cursor", async () => {
    const api = createMockClient();
    const p1 = await api.listIntegrations(MOCK_ORG_ID, { limit: 1 });
    expect(p1.items).toHaveLength(1);
    expect(p1.total).toBe(2);
    const p2 = await api.listIntegrations(MOCK_ORG_ID, { limit: 1, cursor: p1.next_cursor! });
    expect(p2.items).toHaveLength(1);
    expect(p2.next_cursor).toBeNull();
    expect(p2.items[0]!.id).not.toBe(p1.items[0]!.id);
  });

  it("lists active destinations for exporters without config or secrets", async () => {
    const api = createMockClient();
    const full = (await api.listIntegrations(MOCK_ORG_ID)).items;
    await api.updateIntegration(full[0]!.id, { status: "disabled" });
    const { items } = await api.listExportDestinations(MOCK_ORG_ID);
    expect(items).toHaveLength(full.length - 1);
    for (const i of items) expect(Object.keys(i).sort()).toEqual(["environment", "id", "kind", "name", "status"]);
    await expect(api.listExportDestinations("other")).rejects.toMatchObject({ status: 404 });
  });

  it("refuses to delete a destination with pending deliveries (409), then deletes it", async () => {
    const api = createMockClient();
    const dest = (await api.listIntegrations(MOCK_ORG_ID)).items.find((i) => i.kind === "webhook")!;
    const made = await api.createExport(MOCK_ORG_ID, {
      format: "csv", mode: "deidentified", purpose: "research", reason: "Cohort extract for study", destination_ids: [dest.id],
    });
    await expect(api.deleteIntegration(dest.id)).rejects.toMatchObject({ status: 409, slug: "conflict" });
    expect((await api.getIntegration(dest.id)).id).toBe(dest.id);
    const idle = (await api.listIntegrations(MOCK_ORG_ID)).items.find((i) => i.id !== dest.id)!;
    await api.deleteIntegration(idle.id);
    await expect(api.getIntegration(idle.id)).rejects.toMatchObject({ status: 404 });
    await expect(api.deleteIntegration(idle.id)).rejects.toMatchObject({ status: 404 });
    expect((await api.listIntegrations(MOCK_ORG_ID)).items.map((i) => i.id)).toEqual([dest.id]);
    expect((await api.listExportDestinations(MOCK_ORG_ID)).items.map((i) => i.id)).toEqual([dest.id]);
    expect((await api.listExportDeliveries(made.id)).items).toHaveLength(1);
  });
});

describe("audit URL state", () => {
  it("keeps trimmed text filters and dates, turns a day into its UTC bounds and drops garbage", () => {
    const q = parseAuditQuery({
      actor_id: " abc ", action: "integration.", target_type: ["integration", "x"], target_id: "",
      from: "2026-10-01", to: "2026-10-31", cursor: "cur",
    });
    expect(q).toEqual({
      actor_id: "abc", action: "integration.", target_type: "integration",
      from: "2026-10-01T00:00:00.000Z", to: "2026-10-31T23:59:59.999Z", cursor: "cur",
    });
    expect(parseAuditQuery({ from: "not a date", to: "2026-13-45" })).toEqual({});
    expect(parseAuditQuery({ action: "x".repeat(500) }).action).toHaveLength(200);
    // An instant from a pagination link round-trips.
    expect(parseAuditQuery({ from: "2026-10-01T00:00:00.000Z" }).from).toBe("2026-10-01T00:00:00.000Z");
    expect(auditFilterParams(q)).toEqual({
      actor_id: "abc", action: "integration.", target_type: "integration", target_id: undefined,
      from: q.from, to: q.to,
    });
  });
});

describe("components", () => {
  it("AuditFilters is a GET form prefilled from the query, with a clear link", () => {
    const { container } = render(
      <AuditFilters orgId="o 1" query={{ action: "user.", from: "2026-10-01T00:00:00.000Z" }} />,
    );
    const form = container.querySelector("form")!;
    expect(form).toHaveAttribute("method", "get");
    expect(screen.getByLabelText("Action (prefix)")).toHaveValue("user.");
    expect(screen.getByLabelText("From")).toHaveValue("2026-10-01");
    expect(screen.getByLabelText("To")).toHaveValue("");
    for (const name of ["actor_id", "action", "target_type", "target_id", "from", "to"]) {
      expect(form.querySelector(`[name="${name}"]`)).not.toBeNull();
    }
    expect(screen.getByRole("link", { name: "Clear filters" })).toHaveAttribute("href", "/o%201/audit");
  });

  it("ExportForm shows the destination picker from the minimal destination shape", () => {
    render(
      <ExportForm
        create={vi.fn()}
        canCreate
        canIdentified={false}
        destinations={[{ id: "d1", name: "Hospital SFTP", kind: "sftp", environment: "production", status: "active" }]}
        orgId="o"
      />,
    );
    expect(screen.getByLabelText("Hospital SFTP")).toHaveAttribute("name", "destination_ids");
  });

  it("DeleteIntegrationPanel asks in the page before deleting, then returns to the list", async () => {
    const user = userEvent.setup();
    const remove = vi.fn().mockResolvedValue({ ok: true });
    render(<DeleteIntegrationPanel name="EHR hook" remove={remove} listHref="/o/settings/integrations" />);
    await user.click(screen.getByRole("button", { name: "Delete destination" }));
    expect(remove).not.toHaveBeenCalled();
    expect(screen.getByText(/Delete "EHR hook"\?/)).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Cancel" }));
    expect(screen.queryByText(/Delete "EHR hook"\?/)).toBeNull();
    await user.click(screen.getByRole("button", { name: "Delete destination" }));
    await user.click(screen.getByRole("button", { name: "Yes, delete" }));
    expect(remove).toHaveBeenCalledTimes(1);
    expect(await screen.findByRole("status")).toHaveTextContent("Destination deleted.");
    expect(push).toHaveBeenCalledWith("/o/settings/integrations");
  });

  it("DeleteIntegrationPanel explains a 409 and stays on the page", async () => {
    const user = userEvent.setup();
    push.mockClear();
    const remove = vi.fn().mockResolvedValue({ ok: false, slug: "conflict", status: 409 });
    render(<DeleteIntegrationPanel name="EHR hook" remove={remove} listHref="/x" />);
    await user.click(screen.getByRole("button", { name: "Delete destination" }));
    await user.click(screen.getByRole("button", { name: "Yes, delete" }));
    expect(await screen.findByRole("alert")).toHaveTextContent(/still pending/);
    expect(push).not.toHaveBeenCalled();
  });
});
