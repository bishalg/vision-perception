/**
 * WP-C.19 (SHR-7..10, HIP-9): share-a-case dialog, share-link panel, public share page, its download route, the live
 * wire format and the mock's answers. Secrets under test: the passcode is shown once and never kept; the download
 * token is never logged; the public paths are uncached, unindexed and set no session or HIP-2 cookie.
 */
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { NextRequest } from "next/server";
import { afterEach, describe, expect, it, vi } from "vitest";
import { POST as shareDownload } from "@/app/api/share/[id]/route";
import { openShareAction } from "@/app/share/[id]/actions";
import { ShareOpenForm } from "@/app/share/[id]/ShareOpenForm";
import { ShareLinkPanel } from "@/components/exports/ShareLinkPanel";
import { ShareCaseDialog } from "@/components/participant/ShareCaseDialog";
import { ApiError, getApi } from "@/lib/api";
import { createLiveClient } from "@/lib/api/live";
import { createMockClient, MOCK_ORG_ID } from "@/lib/api/mock";
import { MOCK_CASE_EXPORT_ID } from "@/lib/api/mock/exports";
import { MOCK_SHARE_LINKS, MOCK_SHARE_PASSCODE } from "@/lib/api/mock/share";
import { shareLinkState, type CreatedShareLink, type ShareLink } from "@/lib/api/share";
import { resolveShareDownload } from "@/lib/api/shareFile";
import { ACCESS_COOKIE, LAST_SEEN_COOKIE, LOGIN_AT_COOKIE, REFRESH_COOKIE, SESSION_COOKIE_NAMES } from "@/lib/auth/cookies";
import { exportsErrorMessage, sharePageText } from "@/messages/exports";
import { proxy } from "@/proxy";
import nextConfig from "../next.config";

const json = (body: unknown, status = 200, headers: Record<string, string> = {}) =>
  new Response(JSON.stringify(body), { status, headers: { "Content-Type": "application/json", ...headers } });
const problem = (status: number, slug: string) =>
  json({ type: `https://schemas.curv.org/asd/problems/${slug}`, title: slug, status }, status, { "Content-Type": "application/problem+json" });

const link = (over: Partial<ShareLink> = {}): ShareLink => ({
  id: "l1", export_id: "e1", url: "/share/l1", created_at: "2026-11-01T10:00:00Z",
  expires_at: new Date(Date.now() + 86_400_000).toISOString(), max_downloads: 1, downloads: 0, opens: 0,
  revoked_at: null, last_opened_at: null, ...over,
});

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllEnvs();
});

async function code(p: Promise<unknown>): Promise<[number, string | null]> {
  try {
    await p;
  } catch (err) {
    if (err instanceof ApiError) return [err.status, err.slug];
    throw err;
  }
  throw new Error("expected an ApiError");
}

describe("live client wire format (I.13 routes)", () => {
  function client(response: Response, accessToken: string | null = "tok") {
    const f = vi.fn().mockResolvedValue(response);
    return { f, api: createLiveClient({ baseUrl: "https://api.test", accessToken: accessToken ?? undefined, fetchImpl: f as unknown as typeof fetch }) };
  }

  it("creates a share link with POST and the chosen limits", async () => {
    const { f, api } = client(json({ link: link(), passcode: "ABCDEFGHJKLM" }, 201));
    const created = await api.createShareLink("e 1", { expires_in_days: 3, max_downloads: 1 });
    const [url, init] = f.mock.calls[0]!;
    expect(url).toBe("https://api.test/v1/exports/e%201/share-link");
    expect(init.method).toBe("POST");
    expect(JSON.parse(init.body)).toEqual({ expires_in_days: 3, max_downloads: 1 });
    expect(init.headers.Authorization).toBe("Bearer tok");
    expect(created.passcode).toBe("ABCDEFGHJKLM");
  });

  it("lists and revokes links", async () => {
    const list = client(json({ items: [link()], next_cursor: null }));
    await list.api.listShareLinks("e1", { limit: 100 });
    expect(list.f.mock.calls[0]![0]).toBe("https://api.test/v1/exports/e1/share-links?limit=100");
    const del = client(new Response(null, { status: 204 }));
    await del.api.revokeShareLink("l 1");
    expect(del.f.mock.calls[0]![0]).toBe("https://api.test/v1/share-links/l%201");
    expect(del.f.mock.calls[0]![1].method).toBe("DELETE");
  });

  it("opens a link with the passcode in the body and no bearer on a token-less client", async () => {
    const { f, api } = client(
      json({ export_id: "e1", file_name: "c.zip", bytes: 9, expires_at: "2026-11-04T00:00:00Z", downloads_left: 1, download_token: "dt" }),
      null,
    );
    await api.openShareLink("l1", "ABCDEFGHJKLM");
    const [url, init] = f.mock.calls[0]!;
    expect(url).toBe("https://api.test/v1/share/l1/open");
    expect(init.method).toBe("POST");
    expect(JSON.parse(init.body)).toEqual({ passcode: "ABCDEFGHJKLM" });
    expect(init.headers.Authorization).toBeUndefined();
    expect(String(url)).not.toContain("ABCDEFGHJKLM");
  });

  it("maps the backend's refusals to ApiError slugs", async () => {
    expect(await code(client(problem(410, "share-link-expired"), null).api.openShareLink("l", "x"))).toEqual([410, "share-link-expired"]);
    expect(await code(client(problem(401, "share-passcode-invalid"), null).api.openShareLink("l", "x"))).toEqual([401, "share-passcode-invalid"]);
    expect(await code(client(problem(404, "not-found"), null).api.openShareLink("l", "x"))).toEqual([404, "not-found"]);
  });
});

describe("mock share links", () => {
  it("answers 404 for unknown and revoked, 410 for expired and exhausted, 401 for a wrong passcode", async () => {
    const m = createMockClient();
    expect(await code(m.openShareLink("nope", MOCK_SHARE_PASSCODE))).toEqual([404, "not-found"]);
    expect(await code(m.openShareLink(MOCK_SHARE_LINKS.revoked, MOCK_SHARE_PASSCODE))).toEqual([404, "not-found"]);
    expect(await code(m.openShareLink(MOCK_SHARE_LINKS.expired, MOCK_SHARE_PASSCODE))).toEqual([410, "share-link-expired"]);
    expect(await code(m.openShareLink(MOCK_SHARE_LINKS.exhausted, MOCK_SHARE_PASSCODE))).toEqual([410, "share-link-exhausted"]);
    expect(await code(m.openShareLink(MOCK_SHARE_LINKS.active, "WRONGPASS234"))).toEqual([401, "share-passcode-invalid"]);
  });

  it("returns the passcode on create only; the list never carries it; downloads run out; revoke is a 404", async () => {
    const m = createMockClient();
    const { link: l, passcode } = await m.createShareLink(MOCK_CASE_EXPORT_ID, { expires_in_days: 1, max_downloads: 1 });
    expect(JSON.stringify(await m.listShareLinks(MOCK_CASE_EXPORT_ID))).not.toContain(passcode);
    const opened = await m.openShareLink(l.id, passcode);
    expect(opened.downloads_left).toBe(1);
    const file = await m.mockShareFile!(l.id, opened.download_token);
    expect(file.headers.get("content-disposition")).toMatch(/^attachment/);
    expect(await code(m.mockShareFile!(l.id, opened.download_token))).toEqual([410, "share-link-exhausted"]);
    expect(await code(m.openShareLink(l.id, passcode))).toEqual([410, "share-link-exhausted"]);
    const second = await m.createShareLink(MOCK_CASE_EXPORT_ID);
    await m.revokeShareLink(second.link.id);
    expect(await code(m.openShareLink(second.link.id, second.passcode))).toEqual([404, "not-found"]);
    expect((await m.getExport(MOCK_CASE_EXPORT_ID)).share_links_n).toBeGreaterThanOrEqual(6);
  });

  it("refuses links on a non-case export and limits out of range", async () => {
    const m = createMockClient();
    const csv = (await m.listExports(MOCK_ORG_ID)).items.find((e) => e.format === "csv" && e.status === "succeeded")!;
    expect(await code(m.createShareLink(csv.id))).toEqual([409, "conflict"]);
    expect(await code(m.createShareLink(MOCK_CASE_EXPORT_ID, { expires_in_days: 8 }))).toEqual([400, "validation-failed"]);
  });

  it("applies SHR-7 to case requests", async () => {
    const m = createMockClient();
    const base = { format: "case_package" as const, reason: "Second opinion for the family", purpose: "clinical_care" as const };
    const recipient = "Dr Example, Example Children's Hospital";
    expect(
      await code(m.createExport(MOCK_ORG_ID, { ...base, mode: "identified", case: { participant_id: "p", recipient } })),
    ).toEqual([400, "case-consent-missing"]);
    expect(
      await code(m.createExport(MOCK_ORG_ID, { ...base, mode: "deidentified", case: { participant_id: "p", recipient: "short" } })),
    ).toEqual([400, "validation-failed"]);
    expect(
      await code(m.createExport(MOCK_ORG_ID, { ...base, mode: "deidentified", filters: { flag: "red" }, case: { participant_id: "p", recipient } })),
    ).toEqual([400, "validation-failed"]);
    expect(await code(m.createExport(MOCK_ORG_ID, { ...base, mode: "deidentified" }))).toEqual([400, "validation-failed"]);
    const ok = await m.createExport(MOCK_ORG_ID, {
      ...base, mode: "identified",
      case: { participant_id: "p", recipient, consent_reference: "CF-2026-114", consent_recorded_at: "2026-10-30T00:00:00Z" },
    });
    expect(ok.status).toBe("succeeded");
  });

  it("derives link states from the listed fields", () => {
    const now = Date.now();
    expect(shareLinkState(link(), now)).toBe("active");
    expect(shareLinkState(link({ revoked_at: "2026-11-01T00:00:00Z" }), now)).toBe("revoked");
    expect(shareLinkState(link({ expires_at: new Date(now - 1).toISOString() }), now)).toBe("expired");
    expect(shareLinkState(link({ downloads: 1 }), now)).toBe("exhausted");
  });
});

describe("public open action", () => {
  const fd = (passcode: string) => {
    const f = new FormData();
    f.set("passcode", passcode);
    return f;
  };

  it("opens with the passcode (spaces, dashes and case forgiven) and returns no passcode", async () => {
    const r = await openShareAction(MOCK_SHARE_LINKS.active, fd(" mock-pass 2345 "));
    expect(r.ok).toBe(true);
    expect(JSON.stringify(r)).not.toContain(MOCK_SHARE_PASSCODE);
  });

  it("reports the backend's answer as an error key only", async () => {
    expect(await openShareAction(MOCK_SHARE_LINKS.active, fd("WRONGPASS234"))).toEqual({ ok: false, error: "share-passcode-invalid", retryAfter: null });
    expect(await openShareAction(MOCK_SHARE_LINKS.expired, fd(MOCK_SHARE_PASSCODE))).toMatchObject({ error: "share-link-expired" });
    expect(await openShareAction(MOCK_SHARE_LINKS.exhausted, fd(MOCK_SHARE_PASSCODE))).toMatchObject({ error: "share-link-exhausted" });
    expect(await openShareAction(MOCK_SHARE_LINKS.revoked, fd(MOCK_SHARE_PASSCODE))).toMatchObject({ error: "gone" });
    expect(await openShareAction("unknown", fd(MOCK_SHARE_PASSCODE))).toMatchObject({ error: "gone" });
    expect(await openShareAction(MOCK_SHARE_LINKS.active, fd(""))).toMatchObject({ error: "share-passcode-invalid" });
  });
});

describe("download route /api/share/[id]", () => {
  const post = (id: string, token: string) =>
    shareDownload(
      new Request(`http://console.test/api/share/${id}`, {
        method: "POST",
        headers: { "Content-Type": "application/x-www-form-urlencoded" },
        body: new URLSearchParams({ t: token }).toString(),
      }),
      { params: Promise.resolve({ id }) },
    );

  function spyLogs() {
    return (["log", "info", "warn", "error", "debug"] as const).map((k) => vi.spyOn(console, k).mockImplementation(() => undefined));
  }

  it("streams the file as an attachment with no-store and noindex, and never logs the token", async () => {
    const spies = spyLogs();
    const { link: l, passcode } = await getApi().createShareLink(MOCK_CASE_EXPORT_ID, { max_downloads: 1 });
    const { download_token } = await getApi().openShareLink(l.id, passcode);
    const res = await post(l.id, download_token);
    expect(res.status).toBe(200);
    expect(res.headers.get("cache-control")).toBe("no-store");
    expect(res.headers.get("x-robots-tag")).toBe("noindex, nofollow");
    expect(res.headers.get("content-disposition")).toMatch(/^attachment/);
    expect(new Uint8Array(await res.arrayBuffer()).slice(0, 2)).toEqual(new Uint8Array([0x50, 0x4b]));
    // The second download of a one-download link goes back to the page with an error key, uncached.
    const again = await post(l.id, download_token);
    expect(again.status).toBe(303);
    expect(again.headers.get("location")).toBe(`/share/${l.id}?e=share-link-exhausted`);
    expect(again.headers.get("cache-control")).toBe("no-store");
    expect(again.headers.get("x-robots-tag")).toBe("noindex, nofollow");
    for (const s of spies) {
      for (const call of s.mock.calls) expect(JSON.stringify(call)).not.toContain(download_token);
    }
  });

  it("sends a missing or forged token back to the passcode step", async () => {
    const forged = await post(MOCK_SHARE_LINKS.active, "mock-share-token.999999");
    expect(forged.headers.get("location")).toBe(`/share/${MOCK_SHARE_LINKS.active}?e=token`);
    const empty = await post(MOCK_SHARE_LINKS.active, "");
    expect(empty.headers.get("location")).toBe(`/share/${MOCK_SHARE_LINKS.active}?e=token`);
  });

  it("live: fetches the backend file route without a bearer and passes the 410 slug through", async () => {
    const env = { CURV_API_MODE: "live", CURV_API_BASE_URL: "https://api.test/" };
    const f = vi.fn().mockResolvedValue(new Response("PK", { status: 200, headers: { "Content-Type": "application/zip" } }));
    const ok = await resolveShareDownload("l 1", "tok.en", env, f as unknown as typeof fetch);
    expect(ok.kind).toBe("stream");
    const [url, init] = f.mock.calls[0]!;
    expect(url).toBe("https://api.test/v1/share/l%201/file?t=tok.en");
    expect(init.headers).toBeUndefined();
    const gone = vi.fn().mockResolvedValue(problem(410, "export-expired"));
    expect(await resolveShareDownload("l", "tok", env, gone as unknown as typeof fetch)).toEqual({ kind: "error", status: 410, slug: "export-expired" });
    const bad = vi.fn();
    expect(await resolveShareDownload("l", "a b", env, bad as unknown as typeof fetch)).toEqual({ kind: "error", status: 401, slug: null });
    expect(bad).not.toHaveBeenCalled();
    const boom = vi.fn().mockResolvedValue(new Response("x", { status: 500 }));
    expect(await resolveShareDownload("l", "tok", env, boom as unknown as typeof fetch)).toEqual({ kind: "error", status: 502, slug: null });
  });
});

describe("public paths: headers and cookies (HIP-9)", () => {
  it("next.config sends no-store and X-Robots-Tag on /share and /api/share", async () => {
    const rules = await nextConfig.headers!();
    for (const source of ["/share/:path*", "/api/share/:path*"]) {
      const rule = rules.find((r) => r.source === source);
      expect(rule, source).toBeDefined();
      expect(rule!.headers).toEqual(
        expect.arrayContaining([
          { key: "Cache-Control", value: "no-store" },
          { key: "X-Robots-Tag", value: "noindex, nofollow" },
        ]),
      );
    }
  });

  const req = (path: string, cookies: Record<string, string> = {}) => {
    const cookie = Object.entries(cookies).map(([k, v]) => `${k}=${v}`).join("; ");
    return new NextRequest(new URL(path, "http://console.test"), { headers: cookie ? { cookie } : {} });
  };

  it("an anonymous visitor gets the page with the nonce CSP and no cookie at all", async () => {
    const res = await proxy(req("/share/abc"));
    expect(res.headers.get("location")).toBeNull();
    expect(res.headers.get("content-security-policy")).toMatch(/'nonce-[^']+' 'strict-dynamic'/);
    expect(res.cookies.getAll()).toEqual([]);
  });

  it("a signed-in user is not redirected, and no session or HIP-2 cookie is touched (even when idle)", async () => {
    const fresh = { [LAST_SEEN_COOKIE]: String(Date.now() - 60_000), [LOGIN_AT_COOKIE]: String(Date.now() - 3_600_000) };
    const idle = { [LAST_SEEN_COOKIE]: String(Date.now() - 6 * 3_600_000), [LOGIN_AT_COOKIE]: String(Date.now() - 7 * 3_600_000) };
    for (const stamps of [fresh, idle]) {
      for (const path of ["/share/abc", "/api/share/abc"]) {
        const res = await proxy(req(path, { [ACCESS_COOKIE]: "a", [REFRESH_COOKIE]: "r", ...stamps }));
        expect(res.status, path).not.toBe(307);
        expect(res.headers.get("location")).toBeNull();
        const names = res.cookies.getAll().map((c) => c.name);
        for (const n of [...SESSION_COOKIE_NAMES, LAST_SEEN_COOKIE, LOGIN_AT_COOKIE]) expect(names).not.toContain(n);
      }
    }
    // Only the refresh cookie: still no token refresh on a public path.
    const res = await proxy(req("/share/abc", { [REFRESH_COOKIE]: "r", ...fresh }));
    expect(res.cookies.getAll()).toEqual([]);
  });
});

describe("ShareCaseDialog", () => {
  const sessions = vi.fn().mockResolvedValue({ ok: true, sessions: [{ id: "s1", startedAt: "2026-10-01T09:00:00Z" }] });

  async function openDialog(props: Partial<Parameters<typeof ShareCaseDialog>[0]> = {}) {
    const create = vi.fn().mockResolvedValue({ ok: true, id: "e9" });
    render(<ShareCaseDialog canIdentified create={create} loadSessions={sessions} orgPath="/o" {...props} />);
    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: "Share case" }));
    return { user, create, dialog: screen.getByRole("dialog", { name: "Share this case" }) };
  }

  it("refuses identified without a consent reference, before any request, with the API's message", async () => {
    const { user, create, dialog } = await openDialog();
    await user.click(within(dialog).getByRole("radio", { name: "Identified" }));
    await user.type(within(dialog).getByLabelText(/^Recipient/), "Dr Example, Example Children's Hospital");
    await user.type(within(dialog).getByLabelText(/^Reason/), "Second opinion for the family");
    await user.click(within(dialog).getByRole("button", { name: "Create case package" }));
    expect(within(dialog).getByRole("alert")).toHaveTextContent(exportsErrorMessage("case-consent-missing", 400));
    expect(create).not.toHaveBeenCalled();
  });

  it("submits a de-identified case with the chosen sessions and links to the export", async () => {
    const { user, create, dialog } = await openDialog();
    expect(within(dialog).queryByLabelText(/Consent reference/)).toBeNull();
    await user.type(within(dialog).getByLabelText(/^Recipient/), "Dr Example, Example Children's Hospital");
    await user.type(within(dialog).getByLabelText(/^Reason/), "Second opinion for the family");
    await user.click(within(dialog).getByRole("radio", { name: "Choose sessions" }));
    await user.click(await within(dialog).findByRole("checkbox", { name: /2026/ }));
    expect(within(dialog).getByText(/cannot be recalled/)).toBeInTheDocument();
    await user.click(within(dialog).getByRole("button", { name: "Create case package" }));
    const data = create.mock.calls[0]![0] as FormData;
    expect(data.get("mode")).toBe("deidentified");
    expect(data.getAll("session_ids")).toEqual(["s1"]);
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(screen.getByRole("link", { name: "Open the export to share it" })).toHaveAttribute("href", "/o/data/exports/e9");
  });

  it("offers no identified mode to a role without export.identified, and shows the API's refusal", async () => {
    const create = vi.fn().mockResolvedValue({ ok: false, slug: "participant-withdrawn", status: 409 });
    const { user, dialog } = await openDialog({ canIdentified: false, create });
    expect(within(dialog).queryByRole("radio", { name: "Identified" })).toBeNull();
    await user.type(within(dialog).getByLabelText(/^Recipient/), "Dr Example, Example Children's Hospital");
    await user.type(within(dialog).getByLabelText(/^Reason/), "Second opinion for the family");
    await user.click(within(dialog).getByRole("button", { name: "Create case package" }));
    expect(await within(dialog).findByRole("alert")).toHaveTextContent(exportsErrorMessage("participant-withdrawn", 409));
  });
});

describe("ShareLinkPanel", () => {
  const created: CreatedShareLink = { link: link({ id: "new1", url: "/share/new1" }), passcode: "QRSTUVWXYZ23" };

  it("shows the passcode once with the other-channel sentence, and drops it on close", async () => {
    const create = vi.fn().mockResolvedValue({ ok: true, created });
    const user = userEvent.setup();
    const { container } = render(
      <ShareLinkPanel links={[]} ready canManage now={Date.now()} create={create} revoke={vi.fn()} />,
    );
    await user.selectOptions(screen.getByLabelText("Link valid for"), "7");
    await user.click(screen.getByRole("button", { name: "Create share link" }));
    expect((create.mock.calls[0]![0] as FormData).get("expires_in_days")).toBe("7");
    const box = screen.getByTestId("share-created");
    expect(within(box).getByTestId("share-passcode")).toHaveTextContent("QRSTUVWXYZ23");
    expect(within(box).getByTestId("share-url")).toHaveTextContent(/^http.*\/share\/new1$/);
    expect(box).toHaveTextContent(/another channel/);
    await user.click(within(box).getByRole("button", { name: /close/ }));
    expect(container.innerHTML).not.toContain("QRSTUVWXYZ23");
    expect(screen.getByRole("button", { name: "Create share link" })).toBeInTheDocument();
  });

  it("lists states and revokes after a confirmation", async () => {
    const revoke = vi.fn().mockResolvedValue({ ok: true });
    const user = userEvent.setup();
    render(
      <ShareLinkPanel
        links={[link({ id: "a" }), link({ id: "r", revoked_at: "2026-11-01T00:00:00Z" }), link({ id: "x", downloads: 1 })]}
        ready
        canManage
        now={Date.now()}
        create={vi.fn()}
        revoke={revoke}
      />,
    );
    const rows = screen.getAllByRole("row").slice(1);
    expect(rows.map((r) => r.getAttribute("data-state"))).toEqual(["active", "revoked", "exhausted"]);
    expect(within(rows[1]!).queryByRole("button", { name: "Revoke" })).toBeNull();
    await user.click(within(rows[0]!).getByRole("button", { name: "Revoke" }));
    await user.click(within(rows[0]!).getByRole("button", { name: "Yes, revoke" }));
    expect(revoke).toHaveBeenCalledWith("a");
  });

  it("hides the controls from roles without sharelink.manage and before the package is ready", () => {
    const { rerender } = render(<ShareLinkPanel links={[]} ready canManage={false} now={0} create={vi.fn()} revoke={vi.fn()} />);
    expect(screen.queryByRole("button", { name: "Create share link" })).toBeNull();
    rerender(<ShareLinkPanel links={[]} ready={false} canManage now={0} create={vi.fn()} revoke={vi.fn()} />);
    expect(screen.queryByRole("button", { name: "Create share link" })).toBeNull();
    expect(screen.getByText(/once the package is ready/)).toBeInTheDocument();
  });
});

describe("ShareOpenForm (public page)", () => {
  const opened = {
    ok: true as const, fileName: "case-package-x.zip", bytes: 48_211, expiresAt: "2026-11-04T12:00:00Z", downloadsLeft: 1,
    downloadToken: "tok-123",
  };

  it("clears the passcode from the page before the answer and then offers a POST download", async () => {
    let resolve!: (r: typeof opened) => void;
    const open = vi.fn().mockReturnValue(new Promise((r) => (resolve = r)));
    const user = userEvent.setup();
    const { container } = render(<ShareOpenForm linkId="l 1" locale="en" open={open} initialError={null} />);
    const input = screen.getByLabelText("Passcode") as HTMLInputElement;
    expect(input.type).toBe("text");
    await user.type(input, "ABCDEFGHJKLM");
    await user.click(screen.getByRole("button", { name: "Continue" }));
    expect((open.mock.calls[0]![0] as FormData).get("passcode")).toBe("ABCDEFGHJKLM");
    expect(input.value).toBe("");
    resolve(opened);
    const button = await screen.findByRole("button", { name: "Download" });
    const form = button.closest("form")!;
    expect(form.getAttribute("method")).toBe("post");
    expect(form.getAttribute("action")).toBe("/api/share/l%201");
    expect((form.querySelector('input[name="t"]') as HTMLInputElement).value).toBe("tok-123");
    expect(screen.getByTestId("share-file")).toHaveTextContent("case-package-x.zip");
    expect(container.innerHTML).not.toContain("ABCDEFGHJKLM");
  });

  it("shows the backend's refusal and the error key the download route sent back", async () => {
    const open = vi.fn().mockResolvedValue({ ok: false, error: "share-passcode-invalid", retryAfter: null });
    const user = userEvent.setup();
    const { unmount } = render(<ShareOpenForm linkId="l" locale="en" open={open} initialError={null} />);
    await user.type(screen.getByLabelText("Passcode"), "X");
    await user.click(screen.getByRole("button", { name: "Continue" }));
    expect(await screen.findByRole("alert")).toHaveTextContent(sharePageText.errors["share-passcode-invalid"]!);
    unmount();
    render(<ShareOpenForm linkId="l" locale="en" open={open} initialError="share-link-exhausted" />);
    expect(screen.getByRole("alert")).toHaveTextContent(sharePageText.errors["share-link-exhausted"]!);
  });
});
