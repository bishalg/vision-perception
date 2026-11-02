/** WP-C.18: record workspace. Wire format, mock rules, attachment transport, components and RBAC. */
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { AttachmentsPanel } from "@/components/participant/AttachmentsPanel";
import { checkFile, contentDigest } from "@/components/participant/AttachmentUpload";
import { CreateParticipantDialog } from "@/components/participant/CreateParticipantDialog";
import { EncounterForm, dueInstant, localDay, visitInstant } from "@/components/participant/EncounterForm";
import { EncountersPanel, type VisitActions } from "@/components/participant/EncountersPanel";
import { FollowUpsTable, sortFollowUps } from "@/components/participant/FollowUpsTable";
import { ParticipantSearch } from "@/components/participant/ParticipantSearch";
import { describeEntry } from "@/components/participant/Timeline";
import { ROLES } from "@/lib/api";
import { downloadAttachment, uploadAttachmentContent } from "@/lib/api/attachmentFile";
import { MAX_ATTACHMENT_BYTES, attachmentRowFromTimeline, type Encounter, type FollowUp } from "@/lib/api/emr";
import { createLiveClient } from "@/lib/api/live";
import { createMockClient, MOCK_ORG_ID } from "@/lib/api/mock";
import { navLinks } from "@/lib/nav";
import { can } from "@/lib/permissions";
import { formatFileSize, participantErrorMessage } from "@/messages/participant";

vi.mock("next/navigation", () => ({ useRouter: () => ({ refresh: vi.fn() }) }));

const json = (body: unknown, status = 200) =>
  new Response(JSON.stringify(body), { status, headers: { "Content-Type": "application/json" } });
const problem = (status: number, slug: string) =>
  new Response(JSON.stringify({ type: `https://schemas.curv.org/asd/problems/${slug}`, title: slug, status }), {
    status,
    headers: { "Content-Type": "application/problem+json" },
  });

function client(...responses: Response[]) {
  const f = vi.fn();
  for (const r of responses) f.mockResolvedValueOnce(r);
  return { f, api: createLiveClient({ baseUrl: "https://api.test", accessToken: "tok", fetchImpl: f as unknown as typeof fetch }) };
}
const call = (f: ReturnType<typeof vi.fn>, i = 0) => {
  const [url, init] = f.mock.calls[i]! as [string, RequestInit];
  return { url, method: init.method, headers: init.headers as Record<string, string>, body: init.body ? JSON.parse(String(init.body)) : undefined };
};

const encounter = (over: Partial<Encounter> = {}): Encounter => ({
  id: "e1", participant_id: "p1", kind: "follow_up_visit", occurred_at: "2026-10-30T09:00:00.000Z", clinician_id: "u1",
  clinician_display_name: "Dr A", reason: "Repeat observation", summary: null, has_summary: false, status: "open",
  follow_up_due_at: null, follow_up_done_at: null, session_ids: [], row_version: 3, created_at: "2026-10-30T09:00:00.000Z",
  updated_at: null, ...over,
});
const ok = () => Promise.resolve({ ok: true as const });
const actions = (over: Partial<VisitActions> = {}): VisitActions => ({
  create: vi.fn(ok), update: vi.fn(ok), state: vi.fn(ok), session: vi.fn(ok), ...over,
});

afterEach(() => vi.unstubAllGlobals());

describe("RBAC (REC-8)", () => {
  it("participant create and visits for owner, admin, clinician; files read by content roles only", () => {
    const org = ROLES.filter((r) => r !== "superadmin");
    expect(org.filter((r) => can(r, "participant.create")).sort()).toEqual(["admin", "clinician", "owner"]);
    expect(org.filter((r) => can(r, "encounter.write")).sort()).toEqual(["admin", "clinician", "owner"]);
    expect(org.filter((r) => can(r, "attachment.write")).sort()).toEqual(["admin", "clinician", "owner"]);
    expect(can("superadmin", "attachment.write")).toBe(false);
    expect(can("superadmin", "encounter.write")).toBe(true);
    expect(can("admin", "clinical.content")).toBe(false); // admin uploads but cannot open files (acceptance 4)
  });

  it("the nav shows Follow-ups to the roles that record visits", () => {
    const has = (role: (typeof ROLES)[number]) => navLinks("/o", role).some((l) => l.href === "/o/follow-ups");
    expect(has("admin")).toBe(true);
    expect(has("clinician")).toBe(true);
    expect(has("viewer")).toBe(false);
    expect(has("researcher")).toBe(false);
  });
});

describe("live client wire format (B.19 routes)", () => {
  it("creates a participant and searches with a POST body (MRN never in the URL)", async () => {
    const c = client(
      json({ id: "p1", org_id: "o", participant_token: "child-0123456789abcdef0123", session_count: 0, qr_payload: "asd://participant/child-0123456789abcdef0123?org=o" }, 201),
      json({ items: [{ id: "p1", org_id: "o", participant_token: "child-1", session_count: 2, open_encounters_n: 1, next_follow_up_at: null }] }),
    );
    await c.api.createParticipant("o/1", { language: "ne" });
    expect(call(c.f)).toMatchObject({ url: "https://api.test/v1/orgs/o%2F1/participants", method: "POST", body: { language: "ne" } });
    const page = await c.api.searchParticipants("o", { mrn: "MRN-77", has_open_follow_up: true });
    const s = call(c.f, 1);
    expect(s.url).toBe("https://api.test/v1/orgs/o/participants/search");
    expect(s.url).not.toContain("MRN");
    expect(s).toMatchObject({ method: "POST", body: { mrn: "MRN-77", has_open_follow_up: true } });
    expect(page.items[0]!.open_encounters_n).toBe(1);
  });

  it("encounters: list, create, PATCH with If-Match, link and unlink", async () => {
    const e = encounter();
    const c = client(json({ items: [e] }), json(e, 201), json({ ...e, row_version: 4 }), new Response(null, { status: 204 }), new Response(null, { status: 204 }));
    await c.api.listEncounters("p1", { limit: 25 });
    expect(call(c.f).url).toBe("https://api.test/v1/participants/p1/encounters?limit=25");
    await c.api.createEncounter("p1", { kind: "home_visit", occurred_at: "2026-11-01T00:00:00Z" });
    expect(call(c.f, 1)).toMatchObject({ url: "https://api.test/v1/participants/p1/encounters", method: "POST" });
    await c.api.updateEncounter("e/1", 3, { status: "closed" });
    expect(call(c.f, 2)).toMatchObject({ url: "https://api.test/v1/encounters/e%2F1", method: "PATCH", body: { status: "closed" } });
    expect(call(c.f, 2).headers["If-Match"]).toBe('"3"');
    await c.api.linkEncounterSession("e1", "s1");
    expect(call(c.f, 3)).toMatchObject({ url: "https://api.test/v1/encounters/e1/sessions", method: "POST", body: { session_id: "s1" } });
    await c.api.unlinkEncounterSession("e1", "s/1");
    expect(call(c.f, 4)).toMatchObject({ url: "https://api.test/v1/encounters/e1/sessions/s%2F1", method: "DELETE" });
  });

  it("follow-ups take due_before and assigned as query parameters", async () => {
    const c = client(json({ items: [] }));
    await c.api.listFollowUps("o", { due_before: "2026-11-01T00:00:00.000Z", assigned: "me", limit: 50 });
    expect(call(c.f).url).toBe("https://api.test/v1/orgs/o/follow-ups?due_before=2026-11-01T00%3A00%3A00.000Z&assigned=me&limit=50");
  });

  it("attachments: metadata POST, DELETE, and the list from the attachment timeline headers", async () => {
    const meta = { id: "a1", participant_id: "p1", kind: "referral", file_name: "x.pdf", media_type: "application/pdf", byte_size: 9, created_at: "2026-11-01T00:00:00Z" };
    const c = client(
      json({ attachment: meta, upload: { method: "PUT", url: "/v1/attachments/a1/content" } }, 201),
      new Response(null, { status: 204 }),
      json({
        items: [{ kind: "attachment", id: "a1", at: "2026-11-01T00:00:00Z", actor_display_name: "Dr A", summary: { attachment_kind: "referral", file_name: "x.pdf", byte_size: 9 } }],
        next_cursor: null,
      }),
    );
    const up = await c.api.createAttachment("p1", { kind: "referral", file_name: "x.pdf", media_type: "application/pdf", byte_size: 9 });
    expect(up.upload.url).toBe("/v1/attachments/a1/content");
    expect(call(c.f)).toMatchObject({ url: "https://api.test/v1/participants/p1/attachments", method: "POST" });
    await c.api.deleteAttachment("a1");
    expect(call(c.f, 1)).toMatchObject({ url: "https://api.test/v1/attachments/a1", method: "DELETE" });
    const rows = await c.api.listParticipantAttachments("p1");
    expect(call(c.f, 2).url).toBe("https://api.test/v1/participants/p1/timeline?kind=attachment&limit=100");
    expect(rows).toEqual([{ id: "a1", at: "2026-11-01T00:00:00Z", kind: "referral", file_name: "x.pdf", byte_size: 9, uploaded_by: "Dr A" }]);
  });

  it("maps the record problem slugs", async () => {
    const c = client(problem(400, "encounter-summary-forbidden"));
    await expect(c.api.createEncounter("p1", { kind: "other", occurred_at: "2026-11-01T00:00:00Z", summary: "x" })).rejects.toMatchObject({
      status: 400,
      slug: "encounter-summary-forbidden",
    });
  });

  it("attachment rows tolerate either header spelling", () => {
    const base = { kind: "attachment" as const, id: "a", at: "t", session_id: null, actor_id: null, actor_display_name: null };
    expect(attachmentRowFromTimeline({ ...base, summary: { kind: "photo_of_document", name: "n.png", size: 2048 } })).toMatchObject({
      kind: "photo_of_document", file_name: "n.png", byte_size: 2048,
    });
    expect(attachmentRowFromTimeline({ ...base, summary: { kind: "bogus" } })).toMatchObject({ kind: null, file_name: null, byte_size: null });
  });
});

describe("mock record API", () => {
  it("a created participant is listed, opens, searches by code prefix and gets a server-made QR payload", async () => {
    const api = createMockClient();
    await api.getParticipant((await api.listParticipants(MOCK_ORG_ID, { limit: 1 })).items[0]!.id); // warm the cache
    const created = await api.createParticipant(MOCK_ORG_ID, { age_bracket_at_enrol: "24–35 mo" });
    expect(created.participant_token).toMatch(/^child-[0-9a-f]{20}$/);
    expect(created.session_count).toBe(0);
    expect((await api.listParticipants(MOCK_ORG_ID, { limit: 1 })).items[0]!.id).toBe(created.id);
    expect((await api.getParticipant(created.id)).participant_token).toBe(created.participant_token);
    const found = await api.searchParticipants(MOCK_ORG_ID, { token_prefix: created.participant_token.slice(0, 12).toUpperCase() });
    expect(found.items.map((p) => p.id)).toEqual([created.id]);
    expect((await api.searchParticipants(MOCK_ORG_ID, { age_bracket: "24–35 mo" })).items.map((p) => p.id)).toContain(created.id);
  });

  it("encounters: newest first, If-Match, closed rules, links, timeline headers without the summary", async () => {
    const api = createMockClient();
    const p = (await api.listParticipants(MOCK_ORG_ID, { limit: 100 })).items.find((x) => x.session_count > 0)!;
    const session = (await api.listSessions(MOCK_ORG_ID, { participant_id: p.id, limit: 1 })).items[0]!;
    const e = await api.createEncounter(p.id, {
      kind: "home_visit", occurred_at: "2026-11-01T11:00:00Z", reason: "Visit at home", summary: "Private words", session_ids: [session.id],
    });
    expect(e).toMatchObject({ status: "open", has_summary: true, session_ids: [session.id], row_version: 1 });
    const list = await api.listEncounters(p.id);
    expect(list.items[0]!.id).toBe(e.id);
    await expect(api.updateEncounter(e.id, 99, { reason: "x" })).rejects.toMatchObject({ status: 412, slug: "stale-version" });
    const closed = await api.updateEncounter(e.id, 1, { status: "closed" });
    await expect(api.updateEncounter(e.id, closed.row_version, { reason: "late edit" })).rejects.toMatchObject({ status: 409, slug: "encounter-closed" });
    const done = await api.updateEncounter(e.id, closed.row_version, { follow_up_done_at: "2026-11-02T00:00:00Z" });
    expect(done.follow_up_done_at).toBe("2026-11-02T00:00:00.000Z");
    await expect(api.createEncounter(p.id, { kind: "other", occurred_at: "2026-11-01T00:00:00Z", session_ids: ["nope"] })).rejects.toMatchObject({ status: 400 });
    const timeline = await api.getParticipantTimeline(p.id, { kind: ["encounter"], limit: 100 });
    const entry = timeline.items.find((x) => x.id === e.id)!;
    expect(entry.summary).toMatchObject({ encounter_kind: "home_visit", status: "closed" });
    expect(JSON.stringify(timeline)).not.toContain("Private words");
    expect(JSON.stringify(timeline)).not.toContain("Visit at home");
  });

  it("follow-ups: open ones only, earliest due first, filters", async () => {
    const api = createMockClient();
    const all = (await api.listFollowUps(MOCK_ORG_ID)).items;
    expect(all.length).toBeGreaterThan(1);
    const due = all.map((f) => f.follow_up_due_at);
    expect(due).toEqual([...due].sort());
    const overdue = (await api.listFollowUps(MOCK_ORG_ID, { due_before: "2026-11-01T00:00:00Z" })).items;
    expect(overdue.length).toBeGreaterThan(0);
    expect(overdue.every((f) => f.follow_up_due_at < "2026-11-01T00:00:00Z")).toBe(true);
    const first = all[0]!;
    const e = await api.getEncounter(first.encounter_id);
    await api.updateEncounter(e.id, e.row_version, { follow_up_done_at: "2026-11-01T12:00:00Z" });
    expect((await api.listFollowUps(MOCK_ORG_ID)).items.map((f) => f.encounter_id)).not.toContain(first.encounter_id);
  });

  it("attachments: type and size refused like the API, content with Content-Digest, download round trip", async () => {
    const api = createMockClient();
    const env = { CURV_API_MODE: "mock" } as Record<string, string | undefined>;
    const p = (await api.listParticipants(MOCK_ORG_ID, { limit: 100 })).items.find((x) => x.session_count > 0)!;
    await expect(api.createAttachment(p.id, { kind: "report", file_name: "a.gif", media_type: "image/gif" as never, byte_size: 3 })).rejects.toMatchObject({
      slug: "attachment-type-not-allowed",
    });
    await expect(api.createAttachment(p.id, { kind: "report", file_name: "a.pdf", media_type: "application/pdf", byte_size: MAX_ATTACHMENT_BYTES + 1 })).rejects.toMatchObject({
      slug: "attachment-too-large",
    });
    const bytes = new TextEncoder().encode("%PDF-1.1 hello");
    const up = await api.createAttachment(p.id, { kind: "report", file_name: "../My Scan (1).pdf", media_type: "application/pdf", byte_size: bytes.byteLength });
    // As `sanitize_file_name`: unsafe characters become `_`, then leading dots are dropped.
    expect(up.attachment.file_name).toBe("_My_Scan__1_.pdf");
    expect(up.upload).toEqual({ method: "PUT", url: `/v1/attachments/${up.attachment.id}/content` });
    // The route handlers go through the process-wide mock (getApi), so exercise the mock hooks on this client directly.
    const hooks = api as unknown as { mockPutAttachmentContent: (id: string, b: Uint8Array, t: string, d: string | null) => Promise<void> };
    await expect(hooks.mockPutAttachmentContent(up.attachment.id, bytes, "application/pdf", "sha-256=AAAA")).rejects.toMatchObject({ status: 400, slug: "validation-failed" });
    await expect(hooks.mockPutAttachmentContent(up.attachment.id, bytes, "application/pdf", `sha-256=:${btoa("x".repeat(32))}:`)).rejects.toMatchObject({ slug: "digest-mismatch" });
    await hooks.mockPutAttachmentContent(up.attachment.id, bytes, "application/pdf", await contentDigest(bytes.buffer as ArrayBuffer));
    const rows = await api.listParticipantAttachments(p.id);
    expect(rows[0]).toMatchObject({ id: up.attachment.id, kind: "report", byte_size: bytes.byteLength });
    // Through attachmentFile with the shared mock (mock mode): upload then download the same bytes.
    const shared = (await import("@/lib/api")).getApi(undefined, env);
    const q = (await shared.listParticipants(MOCK_ORG_ID, { limit: 100 })).items.find((x) => x.session_count > 0)!;
    const meta = await shared.createAttachment(q.id, { kind: "paper_form", file_name: "f.png", media_type: "image/png", byte_size: 4 });
    const four = new Uint8Array([1, 2, 3, 4]);
    expect(await uploadAttachmentContent(meta.attachment.id, four, "image/png", await contentDigest(four.buffer as ArrayBuffer), "t", env)).toEqual({ kind: "ok" });
    const down = await downloadAttachment(meta.attachment.id, "t", env);
    expect(down).toMatchObject({ kind: "stream", mediaType: "image/png", disposition: 'attachment; filename="f.png"' });
    expect(Array.from((down as { body: Uint8Array }).body)).toEqual([1, 2, 3, 4]);
    await shared.deleteAttachment(meta.attachment.id);
    expect(await downloadAttachment(meta.attachment.id, "t", env)).toEqual({ kind: "error", status: 404, slug: "not-found" });
  });
});

describe("attachment transport (live)", () => {
  const env = { CURV_API_MODE: "live", CURV_API_BASE_URL: "https://api.test/" };

  it("forwards the bytes and Content-Digest with the bearer, and maps the problem slug", async () => {
    const f = vi.fn().mockResolvedValueOnce(new Response(null, { status: 204 })).mockResolvedValueOnce(problem(400, "digest-mismatch"));
    const bytes = new Uint8Array([9, 9]);
    expect(await uploadAttachmentContent("a/1", bytes, "image/png", "sha-256=:abc=:", "tok", env, f as unknown as typeof fetch)).toEqual({ kind: "ok" });
    const [url, init] = f.mock.calls[0]! as [string, RequestInit];
    expect(url).toBe("https://api.test/v1/attachments/a%2F1/content");
    expect(init.method).toBe("PUT");
    expect(init.headers).toMatchObject({ Authorization: "Bearer tok", "Content-Type": "image/png", "Content-Digest": "sha-256=:abc=:" });
    expect(await uploadAttachmentContent("a1", bytes, "image/png", null, "tok", env, f as unknown as typeof fetch)).toEqual({
      kind: "error", status: 400, slug: "digest-mismatch",
    });
  });

  it("refuses more than 10 MiB before calling the backend", async () => {
    const f = vi.fn();
    const big = new Uint8Array(MAX_ATTACHMENT_BYTES + 1);
    expect(await uploadAttachmentContent("a1", big, "image/png", null, "tok", env, f as unknown as typeof fetch)).toEqual({
      kind: "error", status: 400, slug: "attachment-too-large",
    });
    expect(f).not.toHaveBeenCalled();
  });

  it("downloads as an attachment with a safe file name; 403 passes through, 500 becomes 502", async () => {
    const f = vi
      .fn()
      .mockResolvedValueOnce(new Response("img", { status: 200, headers: { "Content-Type": "image/jpeg", "Content-Disposition": 'inline; filename="x y.jpg"' } }))
      .mockResolvedValueOnce(problem(403, "forbidden"))
      .mockResolvedValueOnce(new Response("boom", { status: 500 }));
    const ok = await downloadAttachment("a1", "tok", env, f as unknown as typeof fetch);
    expect(ok).toMatchObject({ kind: "stream", mediaType: "image/jpeg", disposition: 'attachment; filename="x_y.jpg"' });
    expect((f.mock.calls[0]! as [string, RequestInit])[0]).toBe("https://api.test/v1/attachments/a1");
    expect(await downloadAttachment("a1", "tok", env, f as unknown as typeof fetch)).toEqual({ kind: "error", status: 403, slug: "forbidden" });
    expect(await downloadAttachment("a1", "tok", env, f as unknown as typeof fetch)).toEqual({ kind: "error", status: 502, slug: null });
  });
});

describe("route handlers", () => {
  it("the upload handler refuses a body over 10 MiB by its declared length and answers no-store", async () => {
    vi.doMock("@/lib/auth/server", () => ({ accessToken: async () => "tok" }));
    const { PUT } = await import("@/app/api/attachments/[id]/content/route");
    const res = await PUT(
      new Request("http://x/api/attachments/a1/content", { method: "PUT", headers: { "content-length": String(MAX_ATTACHMENT_BYTES + 1) }, body: "x" }),
      { params: Promise.resolve({ id: "a1" }) },
    );
    expect(res.status).toBe(400);
    expect(await res.json()).toEqual({ slug: "attachment-too-large" });
    expect(res.headers.get("cache-control")).toBe("no-store");
    vi.doUnmock("@/lib/auth/server");
  });

  it("the download handler streams with no-store and nosniff (mock mode)", async () => {
    vi.doMock("@/lib/auth/server", () => ({ accessToken: async () => "tok" }));
    const { getApi } = await import("@/lib/api");
    const shared = getApi(undefined, { CURV_API_MODE: "mock" });
    const p = (await shared.listParticipants(MOCK_ORG_ID, { limit: 100 })).items.find((x) => x.session_count > 0)!;
    const file = (await shared.listParticipantAttachments(p.id)).find((a) => a.file_name === "consent-form-scan.png")!;
    const { GET } = await import("@/app/api/attachments/[id]/route");
    const res = await GET(new Request(`http://x/api/attachments/${file.id}`), { params: Promise.resolve({ id: file.id }) });
    expect(res.status).toBe(200);
    expect(res.headers.get("cache-control")).toBe("no-store");
    expect(res.headers.get("x-content-type-options")).toBe("nosniff");
    expect(res.headers.get("content-disposition")).toBe('attachment; filename="consent-form-scan.png"');
    expect(Array.from(new Uint8Array(await res.arrayBuffer()).slice(1, 4))).toEqual([0x50, 0x4e, 0x47]); // "PNG"
    vi.doUnmock("@/lib/auth/server");
  });
});

describe("upload checks in the browser", () => {
  it("Content-Digest is RFC 9530 with the colons", async () => {
    const digest = await contentDigest(new TextEncoder().encode("abc").buffer as ArrayBuffer);
    expect(digest).toBe("sha-256=:ungWv48Bz+pBQUDeXa4iI7ADYaOWF3qctBD/YfIAFa0=:");
  });

  it("refuses over 10 MiB and other types with the same words as the API", () => {
    expect(checkFile({ size: MAX_ATTACHMENT_BYTES + 1, type: "image/png" })).toBe(participantErrorMessage("attachment-too-large", 400));
    expect(checkFile({ size: 10, type: "image/gif" })).toBe(participantErrorMessage("attachment-type-not-allowed", 400));
    expect(checkFile({ size: MAX_ATTACHMENT_BYTES, type: "application/pdf" })).toBeNull();
    expect(participantErrorMessage("attachment-too-large", 400)).toMatch(/10 MB/);
  });

  it("a too-large file never reaches the server action", async () => {
    const user = userEvent.setup({ applyAccept: false });
    const create = vi.fn();
    render(<AttachmentsPanel rows={[]} canRead canWrite visits={[]} actions={{ create, remove: vi.fn(), uploaded: vi.fn() }} />);
    const big = new File(["x"], "big.pdf", { type: "application/pdf" });
    Object.defineProperty(big, "size", { value: MAX_ATTACHMENT_BYTES + 1 });
    await user.upload(screen.getByLabelText(/Choose a file/), big);
    await user.click(screen.getByRole("button", { name: "Upload file" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("The file is too large. The limit is 10 MB.");
    expect(create).not.toHaveBeenCalled();
  });

  it("uploads: metadata, then PUT to the console route with the digest; a failed PUT removes the row", async () => {
    const user = userEvent.setup();
    const create = vi.fn().mockResolvedValue({ ok: true, id: "a/1" });
    const remove = vi.fn().mockResolvedValue({ ok: true });
    const uploaded = vi.fn().mockResolvedValue(undefined);
    const fetchMock = vi
      .fn()
      .mockResolvedValueOnce(new Response(null, { status: 204 }))
      .mockResolvedValueOnce(new Response(JSON.stringify({ slug: "digest-mismatch" }), { status: 400 }));
    vi.stubGlobal("fetch", fetchMock);
    render(<AttachmentsPanel rows={[]} canRead canWrite visits={[{ id: "e1", label: "Visit 1" }]} actions={{ create, remove, uploaded }} />);
    const file = new File(["abc"], "scan.png", { type: "image/png" });
    await user.upload(screen.getByLabelText(/Choose a file/), file);
    await user.selectOptions(screen.getByLabelText("From visit (optional)"), "e1");
    await user.click(screen.getByRole("button", { name: "Upload file" }));
    expect(await screen.findByRole("status")).toHaveTextContent("File uploaded.");
    expect(create).toHaveBeenCalledWith({ kind: "paper_form", file_name: "scan.png", media_type: "image/png", byte_size: 3, encounter_id: "e1" });
    const [url, init] = fetchMock.mock.calls[0]! as [string, RequestInit];
    expect(url).toBe("/api/attachments/a%2F1/content");
    expect(init.method).toBe("PUT");
    expect(init.headers).toEqual({ "Content-Type": "image/png", "Content-Digest": "sha-256=:ungWv48Bz+pBQUDeXa4iI7ADYaOWF3qctBD/YfIAFa0=:" });
    expect(uploaded).toHaveBeenCalled();

    await user.upload(screen.getByLabelText(/Choose a file/), file);
    await user.click(screen.getByRole("button", { name: "Upload file" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("The file was damaged on the way to the server.");
    expect(remove).toHaveBeenCalledWith("a/1");
  });
});

describe("Visits tab", () => {
  const sessions = [{ id: "s1", label: "2026-10-30 09:00 UTC" }];

  it("a clinician sees the summary; an admin sees that one exists, not the text", () => {
    const e = encounter({ summary: "Calm, slept well", has_summary: true });
    const { unmount } = render(
      <EncountersPanel encounters={[e]} sessions={sessions} canWrite canSummary canContent canReopen actions={actions()} now={0} orgPath="/o" />,
    );
    expect(screen.getByTestId("visit-summary")).toHaveTextContent("Calm, slept well");
    unmount();
    render(
      <EncountersPanel encounters={[{ ...e, summary: null }]} sessions={sessions} canWrite canSummary={false} canContent={false} canReopen={false} actions={actions()} now={0} orgPath="/o" />,
    );
    expect(screen.queryByTestId("visit-summary")).toBeNull();
    expect(screen.getByText("A summary is on file. Your role cannot read it.")).toBeInTheDocument();
  });

  it("the admin form has no summary field, and a 400 encounter-summary-forbidden is shown in words", async () => {
    const user = userEvent.setup();
    const create = vi.fn().mockResolvedValue({ ok: false, slug: "encounter-summary-forbidden", status: 400 });
    render(<EncountersPanel encounters={[]} sessions={sessions} canWrite canSummary={false} canContent={false} canReopen={false} actions={actions({ create })} now={0} orgPath="/o" />);
    await user.click(screen.getByRole("button", { name: "Add a visit" }));
    expect(screen.queryByLabelText("Visit summary")).toBeNull();
    expect(screen.getByText("Only clinicians write visit summaries.")).toBeInTheDocument();
    await user.type(screen.getByLabelText("Reason for the visit (short)"), "Check-in");
    await user.click(screen.getByRole("button", { name: "Save visit" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Your role cannot write a visit summary.");
    const fd = create.mock.calls[0]![0] as FormData;
    expect(fd.has("summary")).toBe(false);
    expect(fd.get("kind")).toBe("follow_up_visit");
    expect(Date.parse(String(fd.get("occurred_at")))).not.toBeNaN();
  });

  it("a clinician adds a visit with a summary, a follow-up date and a linked session", async () => {
    const user = userEvent.setup();
    const create = vi.fn().mockResolvedValue({ ok: true });
    render(<EncountersPanel encounters={[]} sessions={sessions} canWrite canSummary canContent canReopen actions={actions({ create })} now={0} orgPath="/o" />);
    await user.click(screen.getByRole("button", { name: "Add a visit" }));
    await user.selectOptions(screen.getByLabelText("Type of visit"), "home_visit");
    await user.type(screen.getByLabelText("Visit summary"), "Home routine reviewed");
    await user.type(screen.getByLabelText("Follow up by (optional)"), "2099-01-31");
    await user.click(screen.getByRole("checkbox", { name: sessions[0]!.label }));
    await user.click(screen.getByRole("button", { name: "Save visit" }));
    expect(await screen.findByRole("status")).toHaveTextContent("Visit saved.");
    const fd = create.mock.calls[0]![0] as FormData;
    expect(fd.get("kind")).toBe("home_visit");
    expect(fd.get("summary")).toBe("Home routine reviewed");
    expect(fd.get("follow_up_due_at")).toBe(new Date("2099-01-31T23:59:00").toISOString());
    expect(fd.getAll("session_ids")).toEqual(["s1"]);
  });

  it("marks an overdue follow-up done, closes, and offers reopen to owner/clinician only", async () => {
    const user = userEvent.setup();
    const state = vi.fn(ok);
    const e = encounter({ follow_up_due_at: "2026-10-31T00:00:00.000Z" });
    const { unmount } = render(
      <EncountersPanel encounters={[e]} sessions={sessions} canWrite canSummary={false} canContent={false} canReopen={false} actions={actions({ state })} now={Date.parse("2026-11-01T00:00:00Z")} orgPath="/o" />,
    );
    expect(screen.getByText("Overdue")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Mark follow-up done" }));
    expect(state).toHaveBeenCalledWith("e1", 3, "follow_up_done");
    await user.click(screen.getByRole("button", { name: "Close visit" }));
    expect(state).toHaveBeenCalledWith("e1", 3, "close");
    unmount();
    const closed = encounter({ status: "closed" });
    const r = render(<EncountersPanel encounters={[closed]} sessions={sessions} canWrite canSummary={false} canContent={false} canReopen={false} actions={actions()} now={0} orgPath="/o" />);
    expect(screen.queryByRole("button", { name: "Reopen visit" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Change" })).toBeNull();
    r.unmount();
    render(<EncountersPanel encounters={[closed]} sessions={sessions} canWrite canSummary canContent canReopen actions={actions({ state })} now={0} orgPath="/o" />);
    await user.click(screen.getByRole("button", { name: "Reopen visit" }));
    expect(state).toHaveBeenCalledWith("e1", 3, "reopen");
  });

  it("a stale save says so and offers a reload", async () => {
    const user = userEvent.setup();
    const update = vi.fn().mockResolvedValue({ ok: false, slug: "stale-version", status: 412 });
    render(<EncountersPanel encounters={[encounter()]} sessions={sessions} canWrite canSummary canContent canReopen actions={actions({ update })} now={0} orgPath="/o" />);
    await user.click(screen.getByRole("button", { name: "Change" }));
    await user.click(screen.getByRole("button", { name: "Save visit" }));
    expect(update).toHaveBeenCalledWith("e1", 3, expect.any(FormData));
    const alert = await screen.findByRole("alert");
    expect(alert).toHaveTextContent("Someone else changed this visit");
  });

  it("links and unlinks sessions of the participant", async () => {
    const user = userEvent.setup();
    const session = vi.fn(ok);
    const two = [...sessions, { id: "s2", label: "2026-10-31 09:00 UTC" }];
    render(<EncountersPanel encounters={[encounter({ session_ids: ["s1"] })]} sessions={two} canWrite canSummary canContent canReopen actions={actions({ session })} now={0} orgPath="/o" />);
    expect(screen.getByRole("link", { name: two[0]!.label })).toHaveAttribute("href", "/o/sessions/s1");
    await user.click(screen.getByRole("button", { name: `Remove session ${two[0]!.label} from this visit` }));
    expect(session).toHaveBeenCalledWith("e1", "s1", false);
    await user.selectOptions(screen.getByLabelText("Link a session"), "s2");
    await user.click(screen.getByRole("button", { name: "Link" }));
    expect(session).toHaveBeenCalledWith("e1", "s2", true);
  });

  it("date helpers: today is now, other days midday local, follow-ups end of day", () => {
    const now = new Date(2026, 10, 3, 9, 30);
    expect(visitInstant(localDay(now), now)).toBe(now.toISOString());
    expect(visitInstant("2026-10-01", now)).toBe(new Date(2026, 9, 1, 12, 0).toISOString());
    expect(dueInstant("2026-11-10")).toBe(new Date(2026, 10, 10, 23, 59).toISOString());
    expect(visitInstant("bad")).toBeNull();
  });

  it("EncounterForm prefills an edit and never offers a summary without canSummary", () => {
    render(<EncounterForm encounter={encounter({ reason: "Old reason" })} sessions={[]} canSummary={false} submit={vi.fn()} onDone={vi.fn()} />);
    expect(screen.getByLabelText("Reason for the visit (short)")).toHaveValue("Old reason");
    expect(screen.queryByLabelText("Visit summary")).toBeNull();
  });
});

describe("Files tab", () => {
  const rows = [{ id: "a/1", at: "2026-11-01T00:00:00Z", kind: "referral" as const, file_name: "letter.pdf", byte_size: 2_500_000, uploaded_by: "Dr A" }];

  it("content roles get a download link through the console route; others do not", () => {
    const { unmount } = render(<AttachmentsPanel rows={rows} canRead canWrite={false} visits={[]} actions={{ create: vi.fn(), remove: vi.fn(), uploaded: vi.fn() }} />);
    expect(screen.getByRole("link", { name: "Download letter.pdf" })).toHaveAttribute("href", "/api/attachments/a%2F1");
    expect(screen.getByText("2.4 MB")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Upload file" })).toBeNull();
    unmount();
    render(<AttachmentsPanel rows={rows} canRead={false} canWrite visits={[]} actions={{ create: vi.fn(), remove: vi.fn(), uploaded: vi.fn() }} />);
    expect(screen.queryByRole("link", { name: /Download/ })).toBeNull();
    expect(screen.getByText("Your role cannot open files.")).toBeInTheDocument();
  });

  it("remove asks first", async () => {
    const user = userEvent.setup();
    const remove = vi.fn(ok);
    render(<AttachmentsPanel rows={rows} canRead canWrite visits={[]} actions={{ create: vi.fn(), remove, uploaded: vi.fn() }} />);
    await user.click(screen.getByRole("button", { name: "Remove letter.pdf" }));
    const dialog = screen.getByRole("alertdialog");
    await user.click(within(dialog).getByRole("button", { name: "Yes, remove" }));
    expect(remove).toHaveBeenCalledWith("a/1");
  });

  it("file sizes read like people write them", () => {
    expect(formatFileSize(512)).toBe("1 KB");
    expect(formatFileSize(340 * 1024)).toBe("340 KB");
    expect(formatFileSize(10_485_760)).toBe("10.0 MB");
    expect(formatFileSize(null)).toBe("-");
  });
});

describe("Follow-ups page", () => {
  const f = (id: string, due: string): FollowUp => ({
    encounter_id: id, participant_id: `p-${id}`, participant_token: `child-${id}`, kind: "phone_call",
    occurred_at: "2026-10-20T00:00:00Z", follow_up_due_at: due, clinician_id: null, clinician_display_name: null, reason: null,
  });

  it("sorts by due date, earliest first, and badges overdue and today", () => {
    const rows = [f("c", "2026-11-09T00:00:00Z"), f("a", "2026-10-30T00:00:00Z"), f("b", "2026-11-01T10:00:00Z")];
    expect(sortFollowUps(rows).map((r) => r.encounter_id)).toEqual(["a", "b", "c"]);
    render(<FollowUpsTable rows={rows} now={Date.parse("2026-11-01T00:00:00Z")} orgPath="/o" canWrite done={vi.fn()} filtered={false} />);
    const trs = screen.getAllByTestId("follow-up");
    expect(trs.map((tr) => within(tr).getByRole("link").textContent)).toEqual(["child-a", "child-b", "child-c"]);
    expect(trs[0]).toHaveAttribute("data-overdue", "true");
    expect(within(trs[1]!).getByText("Due today")).toBeInTheDocument();
    expect(within(trs[0]!).getByRole("link")).toHaveAttribute("href", "/o/participants/p-a?tab=visits");
  });

  it("marks one done and shows a failure in words", async () => {
    const user = userEvent.setup();
    const done = vi.fn().mockResolvedValue({ ok: false, slug: "stale-version", status: 412 });
    render(<FollowUpsTable rows={[f("a", "2026-10-30T00:00:00Z")]} now={0} orgPath="/o" canWrite done={done} filtered={false} />);
    await user.click(screen.getByRole("button", { name: "Mark follow-up for child-a done" }));
    expect(done).toHaveBeenCalledWith("a");
    expect(await screen.findByRole("alert")).toHaveTextContent("Someone else changed this visit");
  });

  it("empty states", () => {
    render(<FollowUpsTable rows={[]} now={0} orgPath="/o" canWrite done={vi.fn()} filtered />);
    expect(screen.getByText("No follow-ups match this filter.")).toBeInTheDocument();
  });
});

describe("New participant and search", () => {
  it("shows the server's code and a QR code, and links the record by id", async () => {
    const user = userEvent.setup();
    const create = vi.fn().mockResolvedValue({
      ok: true,
      participant: { id: "p/9", participant_token: "child-0123456789abcdef0123", qr_payload: "asd://participant/child-0123456789abcdef0123?org=o" },
    });
    render(<CreateParticipantDialog create={create} orgPath="/o" />);
    await user.click(screen.getByRole("button", { name: "New participant" }));
    await user.selectOptions(screen.getByLabelText("Language (optional)"), "ne");
    await user.click(screen.getByRole("button", { name: "Create participant" }));
    expect(await screen.findByTestId("created-token")).toHaveTextContent("child-0123456789abcdef0123");
    expect(screen.getByRole("img", { name: "QR code for participant child-0123456789abcdef0123" })).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Open record" })).toHaveAttribute("href", "/o/participants/p%2F9");
    expect((create.mock.calls[0]![0] as FormData).get("language")).toBe("ne");
    await user.click(screen.getByRole("button", { name: "Close" }));
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("maps a create failure", async () => {
    const user = userEvent.setup();
    render(<CreateParticipantDialog create={vi.fn().mockResolvedValue({ ok: false, slug: "forbidden", status: 403 })} orgPath="/o" />);
    await user.click(screen.getByRole("button", { name: "New participant" }));
    await user.click(screen.getByRole("button", { name: "Create participant" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Your role cannot do this.");
  });

  it("search posts the form to the action, clears the MRN, lists rows and continues with the cursor", async () => {
    const user = userEvent.setup();
    const row = (id: string) => ({ id, org_id: "o", participant_token: `child-${id}`, session_count: 1, open_encounters_n: 2, next_follow_up_at: "2026-11-05T00:00:00Z" });
    const search = vi
      .fn()
      .mockResolvedValueOnce({ ok: true, page: { items: [row("a")], next_cursor: "c1", total: 2 } })
      .mockResolvedValueOnce({ ok: true, page: { items: [row("b")], next_cursor: null, total: 2 } });
    const before = window.location.href;
    render(<ParticipantSearch search={search} canMrn orgPath="/o" />);
    await user.type(screen.getByLabelText("Participant code (start of it)"), "child-");
    await user.type(screen.getByLabelText("Hospital record number (MRN)"), "TU-1");
    await user.click(screen.getByRole("checkbox", { name: "Only participants with a follow-up due" }));
    await user.click(screen.getByRole("button", { name: "Search" }));
    expect(await screen.findByText("1 of 2 found")).toBeInTheDocument();
    const fd = search.mock.calls[0]![0] as FormData;
    expect(fd.get("mrn")).toBe("TU-1");
    expect(fd.get("has_open_follow_up")).toBe("true");
    expect(screen.getByLabelText("Hospital record number (MRN)")).toHaveValue("");
    expect(window.location.href).toBe(before);
    await user.click(screen.getByRole("button", { name: "Show more" }));
    await waitFor(() => expect(screen.getAllByTestId("search-row")).toHaveLength(2));
    expect((search.mock.calls[1]![0] as FormData).get("cursor")).toBe("c1");
    expect(screen.getAllByTestId("search-row")[0]).toHaveTextContent("2");
  });

  it("hides the MRN field without patientref.read", () => {
    render(<ParticipantSearch search={vi.fn()} canMrn={false} orgPath="/o" />);
    expect(screen.queryByLabelText("Hospital record number (MRN)")).toBeNull();
  });
});

describe("timeline headers (REC-5)", () => {
  const base = { session_id: null, actor_id: null, actor_display_name: null, at: "2026-11-01T00:00:00Z", id: "x" };
  it("visit: kind and status; file: kind, name, size; never free text", () => {
    expect(describeEntry({ ...base, kind: "encounter", summary: { encounter_kind: "home_visit", status: "open", reason: "secret" } })).toBe(
      "Home visit (open)",
    );
    expect(describeEntry({ ...base, kind: "attachment", summary: { attachment_kind: "referral", file_name: "r.pdf", byte_size: 2048 } })).toBe(
      "Referral letter, r.pdf, 2 KB",
    );
    expect(describeEntry({ ...base, kind: "encounter", summary: {} })).toBe("Visit");
  });
});
