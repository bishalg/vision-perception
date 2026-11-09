import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { AuditTable } from "@/components/AuditTable";
import { ParticipantsTable } from "@/components/ParticipantsTable";
import { CreateUserForm, UsersTable } from "@/components/UsersAdmin";
import { buildQuery, createLiveClient } from "@/lib/api/live";
import { MOCK_ORG_ID, createMockClient } from "@/lib/api/mock";
import type { AuditEntry, OrgUser, Participant } from "@/lib/api";
import { navItems } from "@/lib/nav";
import { ROLES } from "@/lib/api";
import { assignableRoles, can, canEditUserRole } from "@/lib/permissions";

const user = (n: number, over: Partial<OrgUser> = {}): OrgUser => ({
  id: `u${n}`, email: `u${n}@x.org`, role: "viewer", org_id: "o", status: "active", last_login_at: null,
  display_name: null, professional_registration: null, ...over,
});

describe("admin permissions and navigation", () => {
  it("users and audit are for owner, admin and superadmin only; participants are for everyone", () => {
    for (const a of ["user.read", "user.manage", "audit.read"] as const) {
      expect(ROLES.filter((r) => can(r, a)).sort()).toEqual(["admin", "owner", "superadmin"]);
    }
    const hrefs = (r: Parameters<typeof navItems>[1]) => navItems("/o", r).map((l) => l.href);
    expect(hrefs("viewer")).toContain("/o/clinic/participants");
    expect(hrefs("viewer")).not.toContain("/o/admin/users");
    expect(hrefs("viewer")).not.toContain("/o/data/audit");
    expect(hrefs("clinician")).not.toContain("/o/data/audit");
    expect(hrefs("admin")).toEqual(expect.arrayContaining(["/o/admin/users", "/o/data/audit"]));
    expect(hrefs("owner")).toEqual(expect.arrayContaining(["/o/admin/users", "/o/data/audit"]));
  });

  it("mirrors the REC-8 rows of the backend matrix (phase 7)", () => {
    const allowed = (a: Parameters<typeof can>[1]) => ROLES.filter((r) => can(r, a)).sort();
    for (const a of ["participant.create", "encounter.write"] as const) {
      expect(allowed(a)).toEqual(["admin", "clinician", "owner", "superadmin"]);
    }
    // Attachments are clinical content: never the superadmin (RBAC-3).
    expect(allowed("attachment.write")).toEqual(["admin", "clinician", "owner"]);
    for (const a of ["case.share", "sharelink.manage"] as const) expect(allowed(a)).toEqual(["clinician", "owner"]);
    for (const a of ["audit.export", "user.totp_reset"] as const) {
      expect(allowed(a)).toEqual(["admin", "owner", "superadmin"]);
    }
  });

  it("mirrors can_assign and can_provision: admin grants no owner or clinician; nobody grants superadmin", () => {
    expect(assignableRoles("owner")).toContain("clinician");
    expect(assignableRoles("admin")).not.toContain("clinician");
    expect(assignableRoles("admin")).not.toContain("owner");
    for (const r of ROLES) expect(assignableRoles(r)).not.toContain("superadmin");
    expect(assignableRoles("viewer")).toEqual([]);
    expect(canEditUserRole("admin", "owner")).toBe(false);
    expect(canEditUserRole("admin", "clinician")).toBe(true);
    expect(canEditUserRole("owner", "owner")).toBe(true);
    expect(canEditUserRole("owner", "superadmin")).toBe(false);
  });
});

describe("ParticipantsTable", () => {
  const p = (n: number, over: Partial<Participant> = {}): Participant => ({
    id: `p${n}`, org_id: "o", participant_token: `child-00${n}`, age_bracket_at_enrol: "3-4y", language: "en",
    session_count: n, last_session_at: "2026-11-01T10:00:00.000Z", withdrawn_at: null, ...over,
  });

  it("shows tokens, counts and withdrawn status, and hides the patient-ref column when absent", () => {
    render(<ParticipantsTable participants={[p(1), p(2, { withdrawn_at: "2026-10-01T00:00:00Z" })]} />);
    expect(screen.queryByText("Patient ref")).not.toBeInTheDocument();
    const rows = screen.getAllByRole("row").slice(1);
    expect(within(rows[0]!).getByText("child-001")).toBeInTheDocument();
    expect(within(rows[0]!).getByText("Active")).toHaveClass("badge-green");
    expect(within(rows[1]!).getByText("Withdrawn")).toBeInTheDocument();
  });

  it("shows the patient-ref column when the API sent it, and an empty state", () => {
    render(<ParticipantsTable participants={[p(1, { has_patient_ref: true }), p(2, { has_patient_ref: false })]} />);
    expect(screen.getByText("Patient ref")).toBeInTheDocument();
    expect(screen.getByText("On file")).toBeInTheDocument();
    expect(screen.getByText("None")).toBeInTheDocument();
  });

  it("renders an empty state", () => {
    render(<ParticipantsTable participants={[]} />);
    expect(screen.getByText("No participants yet.")).toBeInTheDocument();
  });
});

describe("UsersTable", () => {
  const base = { selfId: "u1", roles: assignableRoles("admin"), update: vi.fn() };

  it("shows no actions column without user.manage", () => {
    render(<UsersTable {...base} users={[user(1), user(2)]} canManage={false} actor="viewer" />);
    expect(screen.queryByText("Actions")).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Edit" })).not.toBeInTheDocument();
  });

  it("offers Edit on others only: not on yourself, nor on owners for an admin", () => {
    render(
      <UsersTable {...base} users={[user(1), user(2), user(3, { role: "owner" })]} canManage actor="admin" />,
    );
    expect(screen.getAllByRole("button", { name: "Edit" })).toHaveLength(1);
    expect(screen.getByText("(You)")).toBeInTheDocument();
  });

  it("submits role and status and surfaces a problem slug as a message", async () => {
    const update = vi.fn().mockResolvedValue({ ok: false, slug: "last-owner", status: 409 });
    const u = userEvent.setup();
    render(<UsersTable {...base} update={update} users={[user(2)]} canManage actor="owner" roles={assignableRoles("owner")} />);
    await u.click(screen.getByRole("button", { name: "Edit" }));
    await u.selectOptions(screen.getByLabelText("Role"), "clinician");
    await u.selectOptions(screen.getByLabelText("Status"), "disabled");
    await u.click(screen.getByRole("button", { name: "Save" }));
    expect(update).toHaveBeenCalledTimes(1);
    const [id, fd] = update.mock.calls[0] as [string, FormData];
    expect(id).toBe("u2");
    expect(fd.get("role")).toBe("clinician");
    expect(fd.get("status")).toBe("disabled");
    expect(await screen.findByRole("alert")).toHaveTextContent("at least one active owner");
  });
});

describe("CreateUserForm", () => {
  it("lists only the roles the actor may grant and reports success", async () => {
    const create = vi.fn().mockResolvedValue({ ok: true, email: "new@x.org" });
    const u = userEvent.setup();
    render(<CreateUserForm create={create} roles={assignableRoles("admin")} />);
    const options = within(screen.getByLabelText("Role")).getAllByRole("option").map((o) => o.textContent);
    expect(options).toEqual(["Admin", "Researcher", "Reviewer", "Viewer"]);
    await u.type(screen.getByLabelText("Email"), "new@x.org");
    await u.type(screen.getByLabelText(/Password/), "correct horse battery");
    await u.click(screen.getByRole("button", { name: "Create user" }));
    expect(await screen.findByRole("status")).toHaveTextContent("Created new@x.org.");
    expect((create.mock.calls[0]![0] as FormData).get("role")).toBe("viewer");
  });

  it("shows the backend's clinical-role refusal", async () => {
    const create = vi.fn().mockResolvedValue({ ok: false, slug: "clinical-role-not-assignable", status: 403 });
    const u = userEvent.setup();
    render(<CreateUserForm create={create} roles={assignableRoles("admin")} />);
    await u.type(screen.getByLabelText("Email"), "new@x.org");
    await u.type(screen.getByLabelText(/Password/), "correct horse battery");
    await u.click(screen.getByRole("button", { name: "Create user" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Only an owner can create or promote");
  });
});

describe("AuditTable", () => {
  const e = (n: number, over: Partial<AuditEntry> = {}): AuditEntry => ({
    id: String(n), ts: "2026-11-01T10:00:00.000Z", actor_type: "user", actor_id: "abcdef12-0000", org_id: "o",
    action: "user.update", target_type: "user", target_id: "12345678-aaaa", before: { role: "viewer" },
    after: { role: "admin" }, ...over,
  });

  it("renders actor, action, target and redacted before/after as text", () => {
    render(<AuditTable entries={[e(1), e(2, { before: null, after: null, actor_type: "system", actor_id: null })]} />);
    expect(screen.getByText("User abcdef12")).toBeInTheDocument();
    expect(screen.getAllByText("user.update")).toHaveLength(2);
    expect(screen.getAllByText("user 12345678")).toHaveLength(2);
    expect(screen.getByText(/"role": "admin"/)).toBeInTheDocument();
    expect(screen.getByText("System")).toBeInTheDocument();
  });
});

describe("mock admin API", () => {
  const api = createMockClient();

  it("paginates participants, users and audit without overlap", async () => {
    for (const list of [api.listParticipants, api.listUsers, api.listAudit]) {
      const seen = new Set<string>();
      let cursor: string | undefined;
      do {
        const page = await list(MOCK_ORG_ID, { limit: 5, cursor });
        for (const i of page.items) {
          expect(seen.has(i.id)).toBe(false);
          seen.add(i.id);
        }
        cursor = page.next_cursor ?? undefined;
      } while (cursor);
      expect(seen.size).toBeGreaterThan(5);
    }
    await expect(api.listAudit(MOCK_ORG_ID, { cursor: "bogus" })).rejects.toMatchObject({ status: 400 });
    await expect(api.listUsers("other", {})).rejects.toMatchObject({ status: 404 });
  });

  it("enforces create/update rules like the backend: email-taken, validation, self, last owner; audits changes", async () => {
    const { items } = await api.listUsers(MOCK_ORG_ID, { limit: 100 });
    const me = (await api.getMyProfile()).id;
    await expect(api.updateUser(MOCK_ORG_ID, me, { status: "disabled" })).rejects.toMatchObject({ status: 403 });
    const owner = items.find((u) => u.role === "owner")!;
    await expect(api.updateUser(MOCK_ORG_ID, owner.id, { role: "admin" })).rejects.toMatchObject({ status: 409 });
    await expect(api.updateUser(MOCK_ORG_ID, "nope", {})).rejects.toMatchObject({ status: 404 });
    await expect(api.updateUser(MOCK_ORG_ID, items[0]!.id, { status: "invited" })).rejects.toMatchObject({ status: 400 });
    await expect(
      api.createUser(MOCK_ORG_ID, { email: "bad", role: "viewer", password: "short" }),
    ).rejects.toMatchObject({ status: 400 });
    const created = await api.createUser(MOCK_ORG_ID, { email: "New@Demo.example.org", role: "viewer", password: "long enough pw 1" });
    expect(created.email).toBe("new@demo.example.org");
    await expect(
      api.createUser(MOCK_ORG_ID, { email: "new@demo.example.org", role: "viewer", password: "long enough pw 1" }),
    ).rejects.toMatchObject({ status: 409 });
    expect((await api.updateUser(MOCK_ORG_ID, created.id, { role: "reviewer", status: "disabled" })).status).toBe("disabled");
    const audit = (await api.listAudit(MOCK_ORG_ID, { limit: 2 })).items;
    expect(audit.map((a) => a.action)).toEqual(["user.update", "user.create"]);
  });
});

describe("live admin client", () => {
  it("calls the documented routes with cursor and limit and parses the pages", async () => {
    const calls: { url: string; method?: string; body?: string }[] = [];
    const fetchImpl = vi.fn(async (url: string, init: RequestInit) => {
      calls.push({ url, method: init.method, body: init.body as string | undefined });
      const body = url.includes("/audit")
        ? { items: [{ id: "1", ts: "2026-11-01T00:00:00Z", actor_type: "system", action: "x" }] }
        : url.includes("/participants")
          ? { items: [] }
          : { items: [user(1)] };
      return new Response(JSON.stringify(init.method !== "GET" ? user(9) : body), {
        status: 200,
        headers: { "Content-Type": "application/json" },
      });
    });
    const api = createLiveClient({ baseUrl: "https://api.test", accessToken: "t", fetchImpl: fetchImpl as never });
    await api.listParticipants("o 1", { cursor: "abc", limit: 50 });
    await api.listUsers("o", {});
    await api.listAudit("o", { limit: 10 });
    await api.createUser("o", { email: "a@b.org", role: "viewer", password: "pw" });
    await api.updateUser("o", "u/1", { status: "disabled" });
    expect(calls.map((c) => `${c.method} ${c.url}`)).toEqual([
      "GET https://api.test/v1/orgs/o%201/participants?cursor=abc&limit=50",
      "GET https://api.test/v1/orgs/o/users",
      "GET https://api.test/v1/orgs/o/audit?limit=10",
      "POST https://api.test/v1/orgs/o/users",
      "PATCH https://api.test/v1/orgs/o/users/u%2F1",
    ]);
    expect(JSON.parse(calls[4]!.body!)).toEqual({ status: "disabled" });
    expect(buildQuery({ cursor: "", limit: undefined })).toBe("");
  });

  it("maps a problem+json 409 to an ApiError with its slug", async () => {
    const fetchImpl = async () =>
      new Response(
        JSON.stringify({ type: "https://schemas.curv.org/asd/problems/last-owner", title: "Conflict", status: 409 }),
        { status: 409, headers: { "Content-Type": "application/problem+json" } },
      );
    const api = createLiveClient({ baseUrl: "https://api.test", fetchImpl: fetchImpl as never });
    await expect(api.updateUser("o", "u", { role: "admin" })).rejects.toMatchObject({ status: 409, slug: "last-owner" });
  });
});
