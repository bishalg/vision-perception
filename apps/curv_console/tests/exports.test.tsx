/** WP-I.8: exports and integration settings. Wire format, mock client rules, components, RBAC and the download route. */
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { DeliveriesTable } from "@/components/exports/DeliveriesTable";
import { ExportForm } from "@/components/exports/ExportForm";
import { ExportsTable } from "@/components/exports/ExportsTable";
import { IntegrationForm } from "@/components/exports/IntegrationForm";
import { SecretPanel, SshKeyPanel } from "@/components/exports/IntegrationPanels";
import { IntegrationsTable } from "@/components/exports/IntegrationsTable";
import { ApiError, ROLES } from "@/lib/api";
import { resolveExportDownload } from "@/lib/api/exportFile";
import type { Export, Integration } from "@/lib/api/exports";
import { createLiveClient } from "@/lib/api/live";
import { createMockClient, MOCK_ORG_ID } from "@/lib/api/mock";
import { navLinks } from "@/lib/nav";
import { can } from "@/lib/permissions";

const json = (body: unknown, status = 200) =>
  new Response(JSON.stringify(body), { status, headers: { "Content-Type": "application/json" } });

const exp = (over: Partial<Export> = {}): Export => ({
  id: "4f0e7a52-6a8c-4c3e-9a51-2f6b0c1d9e01", org_id: "o", kind: "bulk", format: "csv", mode: "deidentified",
  purpose: "research", status: "succeeded", filters: {}, reason: "Quarterly research extract", destination_ids: [],
  created_at: "2026-10-31T09:00:00.000Z", excluded: { no_research_consent: 3, withdrawn: 1 }, row_count: 42, ...over,
});

const integration = (over: Partial<Integration> = {}): Integration => ({
  id: "i1", org_id: "o", kind: "webhook", name: "EHR hook", environment: "test", status: "active",
  config: { url: "https://x.example.org/h" }, has_secret: true, secret_fingerprint: "1a2b3c4d",
  created_at: "2026-10-31T09:00:00.000Z", updated_at: "2026-10-31T09:00:00.000Z", ...over,
});

describe("live client wire format", () => {
  function client(response: Response) {
    const f = vi.fn().mockResolvedValue(response);
    return { f, api: createLiveClient({ baseUrl: "https://api.test", accessToken: "tok", fetchImpl: f as unknown as typeof fetch }) };
  }

  it("lists exports with an opaque cursor and limit", async () => {
    const { f, api } = client(json({ items: [exp()], next_cursor: "abc", total: 9 }));
    const page = await api.listExports("org 1", { cursor: "c1", limit: 50 });
    expect(f.mock.calls[0]![0]).toBe("https://api.test/v1/orgs/org%201/exports?cursor=c1&limit=50");
    expect(page.next_cursor).toBe("abc");
  });

  it("creates an export with POST and the request body unchanged", async () => {
    const { f, api } = client(json(exp({ status: "queued" }), 202));
    await api.createExport("o", { format: "csv", mode: "deidentified", purpose: "research", reason: "A long enough reason", filters: { from: "2026-10-01" } });
    const [url, init] = f.mock.calls[0]!;
    expect(url).toBe("https://api.test/v1/orgs/o/exports");
    expect(init.method).toBe("POST");
    expect(JSON.parse(init.body)).toEqual({ format: "csv", mode: "deidentified", purpose: "research", reason: "A long enough reason", filters: { from: "2026-10-01" } });
  });

  it("requests a download URL with POST", async () => {
    const { f, api } = client(json({ url: "/v1/exports/x/file?t=tok", expires_at: "2026-10-31T09:05:00.000Z" }));
    await api.createExportDownloadUrl("x");
    expect(f.mock.calls[0]![0]).toBe("https://api.test/v1/exports/x/download-url");
    expect(f.mock.calls[0]![1].method).toBe("POST");
  });

  it("uses the secret, ssh-key, test and delivery paths", async () => {
    const calls: [string, string][] = [];
    const f = vi.fn(async (url: string, init: RequestInit) => {
      calls.push([init.method ?? "GET", url.replace("https://api.test/v1", "")]);
      if (url.endsWith("/secret")) return json(integration());
      if (url.endsWith("/ssh-key")) return json({ public_key: "ssh-ed25519 AAAA", fingerprint: "SHA256:x" });
      if (url.endsWith("/test")) return json({ ok: true, tested_at: "2026-10-31T09:00:00.000Z" });
      return json({ items: [], next_cursor: null, total: 0 });
    });
    const api = createLiveClient({ baseUrl: "https://api.test", accessToken: "t", fetchImpl: f as unknown as typeof fetch });
    await api.putIntegrationSecret("i", "whsec_abc");
    await api.generateIntegrationSshKey("i");
    await api.testIntegration("i");
    await api.listIntegrationDeliveries("i", { limit: 10 });
    await api.listExportDeliveries("e");
    expect(calls).toEqual([
      ["PUT", "/integrations/i/secret"],
      ["POST", "/integrations/i/ssh-key"],
      ["POST", "/integrations/i/test"],
      ["GET", "/integrations/i/deliveries?limit=10"],
      ["GET", "/exports/e/deliveries"],
    ]);
    expect(JSON.parse(f.mock.calls[0]![1].body as string)).toEqual({ secret: "whsec_abc" });
  });

  it("raises ApiError with the problem slug (consent-purpose-mismatch)", async () => {
    const { api } = client(json({ type: "https://schemas.curv.org/asd/problems/consent-purpose-mismatch", title: "x", status: 400 }, 400));
    await expect(api.createExport("o", { format: "csv", mode: "identified", purpose: "research", reason: "A long enough reason" }))
      .rejects.toMatchObject({ status: 400, slug: "consent-purpose-mismatch" });
  });

  it("rejects an integration response that violates the contract", async () => {
    const { api } = client(json({ id: "i", kind: "carrier-pigeon" }));
    await expect(api.getIntegration("i")).rejects.toThrow();
  });
});

describe("mock client", () => {
  const request = { format: "csv", mode: "deidentified", purpose: "research", reason: "Cohort extract for study" } as const;

  it("creates an export, queues one delivery per destination and lists it first", async () => {
    const api = createMockClient();
    const dest = (await api.listIntegrations(MOCK_ORG_ID)).items[0]!;
    const created = await api.createExport(MOCK_ORG_ID, { ...request, destination_ids: [dest.id] });
    expect(created.status).toBe("queued");
    expect((await api.listExports(MOCK_ORG_ID)).items[0]!.id).toBe(created.id);
    const log = await api.listExportDeliveries(created.id);
    expect(log.items).toHaveLength(1);
    expect(log.items[0]).toMatchObject({ status: "pending", destination_id: dest.id });
  });

  it("answers validation-failed, consent-purpose-mismatch and the download states like the backend", async () => {
    const api = createMockClient();
    await expect(api.createExport(MOCK_ORG_ID, { ...request, reason: "short" })).rejects.toMatchObject({ slug: "validation-failed" });
    await expect(api.createExport(MOCK_ORG_ID, { ...request, mode: "identified" })).rejects.toMatchObject({ slug: "consent-purpose-mismatch" });
    const all = (await api.listExports(MOCK_ORG_ID, { limit: 100 })).items;
    const by = (s: string) => all.find((e) => e.status === s)!;
    await expect(api.createExportDownloadUrl(by("expired").id)).rejects.toMatchObject({ status: 410, slug: "export-expired" });
    await expect(api.createExportDownloadUrl(by("queued").id)).rejects.toMatchObject({ status: 409 });
    expect((await api.createExportDownloadUrl(by("succeeded").id)).url).toMatch(/^\/v1\/exports\/.+\/file\?t=/);
  });

  it("paginates exports with an opaque cursor", async () => {
    const api = createMockClient();
    const first = await api.listExports(MOCK_ORG_ID, { limit: 2 });
    expect(first.items).toHaveLength(2);
    expect(first.next_cursor).toBeTruthy();
    const second = await api.listExports(MOCK_ORG_ID, { limit: 2, cursor: first.next_cursor! });
    expect(second.items.map((e) => e.id)).not.toContain(first.items[0]!.id);
    await expect(api.listExports(MOCK_ORG_ID, { cursor: "garbage" })).rejects.toBeInstanceOf(ApiError);
  });

  it("never returns a secret: the secret route answers has_secret and a fingerprint only", async () => {
    const api = createMockClient();
    const created = await api.createIntegration(MOCK_ORG_ID, { kind: "webhook", name: "New hook", config: { url: "https://h.example.org/x" } });
    expect(created.has_secret).toBe(false);
    const secret = `whsec_${btoa("x".repeat(32))}`;
    const after = await api.putIntegrationSecret(created.id, secret);
    expect(after.has_secret).toBe(true);
    expect(JSON.stringify(after)).not.toContain(secret);
    await expect(api.putIntegrationSecret(created.id, "nope")).rejects.toMatchObject({ slug: "validation-failed" });
  });

  it("generates an ssh key only for sftp destinations and reports test outcomes as error classes", async () => {
    const api = createMockClient();
    const list = (await api.listIntegrations(MOCK_ORG_ID)).items;
    const hook = list.find((i) => i.kind === "webhook")!;
    const sftp = list.find((i) => i.kind === "sftp")!;
    await expect(api.generateIntegrationSshKey(hook.id)).rejects.toMatchObject({ status: 409 });
    const key = await api.generateIntegrationSshKey(sftp.id);
    expect(key.public_key).toMatch(/^ssh-ed25519 /);
    expect((await api.getIntegration(sftp.id)).ssh_public_key).toBe(key.public_key);
    const fresh = await api.createIntegration(MOCK_ORG_ID, { kind: "webhook", name: "No secret", config: { url: "https://h.example.org/x" } });
    expect(await api.testIntegration(fresh.id)).toMatchObject({ ok: false, error_class: "missing-secret" });
  });

  it("edits a destination and refuses an invalid config", async () => {
    const api = createMockClient();
    const hook = (await api.listIntegrations(MOCK_ORG_ID)).items.find((i) => i.kind === "webhook")!;
    const next = await api.updateIntegration(hook.id, { name: "Renamed", status: "disabled", config: { url: "https://h.example.org/y" } });
    expect(next).toMatchObject({ name: "Renamed", status: "disabled", config: { url: "https://h.example.org/y" } });
    await expect(api.updateIntegration(hook.id, { config: { url: "http://insecure" } })).rejects.toMatchObject({ slug: "validation-failed" });
    await expect(api.createIntegration(MOCK_ORG_ID, { kind: "fhir_server", name: "FHIR" })).rejects.toMatchObject({ slug: "validation-failed" });
  });

  it("404s an unknown organisation and unknown ids", async () => {
    const api = createMockClient();
    await expect(api.listExports("nope")).rejects.toMatchObject({ status: 404 });
    await expect(api.getExport("nope")).rejects.toMatchObject({ status: 404 });
    await expect(api.getIntegration("nope")).rejects.toMatchObject({ status: 404 });
  });
});

describe("RBAC and navigation", () => {
  it("mirrors the backend matrix", () => {
    const allowed = (a: Parameters<typeof can>[1]) => ROLES.filter((r) => can(r, a));
    expect(allowed("export.read")).toEqual(["owner", "admin", "clinician", "researcher", "superadmin"]);
    expect(allowed("export.create")).toEqual(["owner", "admin", "clinician", "researcher"]);
    expect(allowed("export.download")).toEqual(["owner", "admin", "clinician", "researcher"]);
    expect(allowed("export.identified")).toEqual(["owner", "clinician"]);
    expect(allowed("integration.read")).toEqual(["owner", "admin", "superadmin"]);
    expect(allowed("integration.manage")).toEqual(["owner", "admin"]);
  });

  it("shows the exports and integrations links by role", () => {
    const hrefs = (r: Parameters<typeof navLinks>[1]) => navLinks("/o", r).map((l) => l.href);
    expect(hrefs("researcher")).toContain("/o/exports");
    expect(hrefs("researcher")).not.toContain("/o/settings/integrations");
    expect(hrefs("admin")).toEqual(expect.arrayContaining(["/o/exports", "/o/settings/integrations"]));
    expect(hrefs("viewer")).not.toContain("/o/exports");
  });
});

describe("components", () => {
  it("ExportsTable shows status, excluded counts, a details link and a POST download only for ready exports", () => {
    const { container } = render(
      <ExportsTable orgId="org-1" canDownload items={[exp(), exp({ id: "e2", status: "expired" }), exp({ id: "e3", status: "failed", row_count: null, excluded: {} })]} />,
    );
    const rows = screen.getAllByRole("row").slice(1);
    expect(within(rows[0]!).getByText("Ready")).toHaveClass("badge-green");
    expect(within(rows[0]!).getByText(/No research consent 3, Withdrawn 1|Withdrawn 1, No research consent 3/)).toBeInTheDocument();
    expect(within(rows[0]!).getByRole("link", { name: "Details" })).toHaveAttribute("href", "/org-1/exports/4f0e7a52-6a8c-4c3e-9a51-2f6b0c1d9e01");
    expect(within(rows[1]!).getByText("Expired")).toBeInTheDocument();
    expect(within(rows[2]!).getByText("Failed")).toHaveClass("badge-red");
    const forms = container.querySelectorAll("form");
    expect(forms).toHaveLength(1);
    expect(forms[0]).toHaveAttribute("method", "post");
    expect(forms[0]).toHaveAttribute("action", "/api/export/4f0e7a52-6a8c-4c3e-9a51-2f6b0c1d9e01");
  });

  it("ExportsTable hides downloads without permission and shows an empty state", () => {
    const { container, rerender } = render(<ExportsTable orgId="o" canDownload={false} items={[exp()]} />);
    expect(container.querySelector("form")).toBeNull();
    rerender(<ExportsTable orgId="o" canDownload items={[]} />);
    expect(screen.getByText("No exports yet.")).toBeInTheDocument();
  });

  it("ExportForm offers identified mode only to roles that hold it, and hides PDF summary", () => {
    const create = vi.fn();
    const { rerender } = render(<ExportForm create={create} canCreate canIdentified={false} destinations={[]} orgId="o" />);
    expect(screen.queryByRole("option", { name: "Identified" })).toBeNull();
    expect(screen.queryByRole("option", { name: /PDF summary/ })).toBeNull();
    rerender(<ExportForm create={create} canCreate canIdentified destinations={[]} orgId="o" />);
    expect(screen.getByRole("option", { name: "Identified" })).toBeInTheDocument();
    rerender(<ExportForm create={create} canCreate={false} canIdentified={false} destinations={[]} orgId="o" />);
    expect(screen.getByText("Your role cannot create exports.")).toBeInTheDocument();
  });

  it("ExportForm submits, links to the queued export and shows the backend's reason on failure", async () => {
    const user = userEvent.setup();
    const create = vi.fn().mockResolvedValueOnce({ ok: false, slug: "consent-purpose-mismatch", status: 400, params: "mode: x" })
      .mockResolvedValueOnce({ ok: true, id: "new-id" });
    render(<ExportForm create={create} canCreate canIdentified destinations={[{ ...integration(), id: "d1" }]} orgId="org-1" />);
    await user.type(screen.getByLabelText(/^Reason/), "Cohort extract for the study");
    await user.click(screen.getByLabelText("EHR hook"));
    await user.click(screen.getByRole("button", { name: "Request export" }));
    expect(await screen.findByRole("alert")).toHaveTextContent(/Identified exports are limited.*mode: x/);
    const fd = create.mock.calls[0]![0] as FormData;
    expect(fd.get("format")).toBe("csv");
    expect(fd.getAll("destination_ids")).toEqual(["d1"]);
    // React resets the form after an action, so the required reason is typed again.
    await user.type(screen.getByLabelText(/^Reason/), "Cohort extract for the study");
    await user.click(screen.getByRole("button", { name: "Request export" }));
    expect(await screen.findByRole("link", { name: "View export" })).toHaveAttribute("href", "/org-1/exports/new-id");
  });

  it("DeliveriesTable shows status, attempts and the error class", () => {
    render(
      <DeliveriesTable
        items={[{
          id: "d", export_id: "4f0e7a52-0000", destination_id: "x", destination_name: "Hospital SFTP", event: "export.ready",
          status: "failed", attempts: 5, created_at: "2026-10-31T09:00:00.000Z", updated_at: "2026-10-31T09:10:00.000Z",
          last_error_class: "connect-failed",
        }]}
        exportHref={(id) => `/o/exports/${id}`}
      />,
    );
    const row = screen.getAllByRole("row")[1]!;
    expect(within(row).getByText("Failed")).toHaveClass("badge-red");
    expect(within(row).getByText("connect-failed")).toBeInTheDocument();
    expect(within(row).getByRole("link", { name: "4f0e7a52" })).toHaveAttribute("href", "/o/exports/4f0e7a52-0000");
  });

  it("IntegrationsTable shows credential and last-test state without any secret", () => {
    const { container } = render(
      <IntegrationsTable orgId="o" items={[
        integration({ last_test_at: "2026-10-31T09:00:00.000Z", last_test_ok: false, last_test_error: "connect-failed" }),
        integration({ id: "i2", name: "Drop", kind: "sftp", has_secret: false, secret_fingerprint: null }),
      ]} />,
    );
    expect(screen.getByText("Failed")).toHaveClass("badge-red");
    expect(screen.getByText(/connect-failed/)).toBeInTheDocument();
    expect(screen.getByText("Missing")).toBeInTheDocument();
    expect(screen.getByText("Never run")).toBeInTheDocument();
    expect(container.textContent).not.toContain("1a2b3c4d");
  });

  it("IntegrationForm switches fields by kind and prefills an existing destination", async () => {
    const user = userEvent.setup();
    const save = vi.fn().mockResolvedValue({ ok: true, id: "new" });
    const { unmount } = render(<IntegrationForm save={save} orgId="o" />);
    expect(screen.getByLabelText("Receiver URL (https only)")).toBeInTheDocument();
    await user.selectOptions(screen.getByLabelText("Kind"), "sftp");
    expect(screen.getByLabelText("Host")).toBeInTheDocument();
    expect(screen.queryByLabelText("Receiver URL (https only)")).toBeNull();
    await user.type(screen.getByLabelText("Name"), "Drop");
    await user.click(screen.getByRole("button", { name: "Add destination" }));
    expect(await screen.findByRole("link", { name: "Open it to set up its credential." })).toHaveAttribute("href", "/o/settings/integrations/new");
    unmount();
    render(<IntegrationForm save={save} orgId="o" existing={integration({ config: { url: "https://x.example.org/h", events: ["review.finalized"] } })} />);
    expect(screen.getByLabelText("Receiver URL (https only)")).toHaveValue("https://x.example.org/h");
    expect(screen.getByLabelText("Review finalized")).toBeChecked();
    expect(screen.getByLabelText("Export ready")).not.toBeChecked();
    expect(screen.queryByLabelText("Kind")).toBeNull();
  });

  it("SecretPanel shows the fingerprint only, shows a generated secret once, and never echoes a pasted one", async () => {
    const user = userEvent.setup();
    const put = vi.fn().mockResolvedValueOnce({ ok: true, generated: "whsec_GENERATED" }).mockResolvedValueOnce({ ok: true });
    const { container } = render(<SecretPanel integration={{ has_secret: true, secret_fingerprint: "1a2b3c4d" }} put={put} canManage />);
    expect(screen.getByText("A secret is set (fingerprint 1a2b3c4d).")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Rotate secret" }));
    expect(await screen.findByTestId("generated-secret")).toHaveTextContent("whsec_GENERATED");
    await user.type(screen.getByLabelText(/Use your own secret/), "whsec_PASTED");
    await user.click(screen.getByRole("button", { name: "Rotate secret" }));
    await screen.findByText("Secret saved.");
    expect(screen.queryByTestId("generated-secret")).toBeNull();
    expect(container.textContent).not.toContain("whsec_PASTED");
  });

  it("SecretPanel hides the form from read-only roles", () => {
    render(<SecretPanel integration={{ has_secret: false }} put={vi.fn()} canManage={false} />);
    expect(screen.getByText(/No secret is set/)).toBeInTheDocument();
    expect(screen.queryByRole("button")).toBeNull();
  });

  it("SshKeyPanel shows the stored public key and asks before rotating", async () => {
    const user = userEvent.setup();
    const generate = vi.fn().mockResolvedValue({ ok: true, key: { public_key: "ssh-ed25519 NEWKEY", fingerprint: "SHA256:new" } });
    render(<SshKeyPanel integration={{ ssh_public_key: "ssh-ed25519 OLDKEY" }} generate={generate} canManage />);
    expect(screen.getByTestId("ssh-public-key")).toHaveTextContent("ssh-ed25519 OLDKEY");
    await user.click(screen.getByRole("button", { name: "Rotate key" }));
    expect(generate).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "Rotate key" }));
    expect(await screen.findByText("SHA256:new")).toBeInTheDocument();
    expect(screen.getByTestId("ssh-public-key")).toHaveTextContent("ssh-ed25519 NEWKEY");
  });
});

describe("export download route logic", () => {
  const mockEnv = { CURV_API_MODE: "mock" };

  it("mock mode streams a placeholder file (the relative signed URL is never exposed)", async () => {
    const api = createMockClient();
    const ready = (await api.listExports(MOCK_ORG_ID, { limit: 100 })).items.find((e) => e.status === "succeeded")!;
    const out = await resolveExportDownload(ready.id, "tok", mockEnv);
    expect(out.kind).toBe("stream");
  });

  it("maps backend failures to browser statuses", async () => {
    const out = await resolveExportDownload("00000000-0000-4000-8000-000000000000", "tok", mockEnv);
    expect(out).toEqual({ kind: "error", status: 404 });
  });

  it("live: a presigned URL is a redirect; a proxy URL is fetched from the backend; odd paths are refused", async () => {
    const env = { CURV_API_MODE: "live", CURV_API_BASE_URL: "https://api.test" };
    const download = (url: string) =>
      vi.fn().mockResolvedValueOnce(json({ url, expires_at: "2026-10-31T09:05:00.000Z" }))
        .mockResolvedValueOnce(new Response("zipbytes", { status: 200, headers: { "Content-Type": "application/zip" } }));
    // The shared API client uses global fetch; stub it for the duration of each call.
    const run = async (url: string) => {
      const f = download(url);
      vi.stubGlobal("fetch", f);
      try {
        return { out: await resolveExportDownload("e1", "tok", env, f as unknown as typeof fetch), f };
      } finally {
        vi.unstubAllGlobals();
      }
    };
    const presigned = await run("https://blob.example.org/x?sig=1");
    expect(presigned.out).toEqual({ kind: "redirect", location: "https://blob.example.org/x?sig=1" });
    const proxy = await run("/v1/exports/e1/file?t=abc");
    expect(proxy.out.kind).toBe("stream");
    expect(proxy.f.mock.calls[1]![0]).toBe("https://api.test/v1/exports/e1/file?t=abc");
    expect((await run("/v1/admin/secrets")).out).toEqual({ kind: "error", status: 502 });
  });
});
