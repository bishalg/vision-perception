/** WP-C.18 TRS-3: admin reset of a user's two-step sign-in (UI-3 step-up dialog), mock rules and the live route. */
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { TotpResetDialog, UsersTable, totpResetErrorText, type TotpResetResult } from "@/components/UsersAdmin";
import { ROLES, type OrgUser, type Role } from "@/lib/api";
import { createLiveClient } from "@/lib/api/live";
import { adminMessages } from "@/messages/admin";
import { createMockClient, MOCK_ORG_ID, MOCK_PASSWORD } from "@/lib/api/mock";
import { MOCK_STEP_UP_PASSWORD } from "@/lib/api/mock/admin";
import { assignableRoles, can } from "@/lib/permissions";

const user = (n: number, over: Partial<OrgUser> = {}): OrgUser => ({
  id: `u${n}`, email: `user${n}@demo.example.org`, role: "clinician", org_id: "o", status: "active",
  last_login_at: null, display_name: null, professional_registration: null, ...over,
});
const fail = (slug: string | null, status: number): TotpResetResult => ({ ok: false, slug, status });

describe("who may reset", () => {
  it("owner and admin only (UserTotpReset), never the superadmin short-circuit into other orgs' clinical actions", () => {
    expect(ROLES.filter((r) => r !== "superadmin" && can(r, "user.totp_reset")).sort()).toEqual(["admin", "owner"]);
  });

  function table(actor: Role, reset = vi.fn()) {
    return render(
      <UsersTable
        users={[user(1, { role: "owner" }), user(2, { role: "admin" }), user(3), user(9, { role: "admin" })]}
        selfId="u9"
        canManage
        actor={actor}
        roles={assignableRoles(actor)}
        update={vi.fn()}
        resetTotp={reset}
      />,
    );
  }

  it("the button is hidden for yourself and for targets above your rank", () => {
    table("admin");
    const buttons = screen.getAllByRole("button", { name: /^Reset two-step sign-in for/ });
    expect(buttons.map((b) => b.getAttribute("aria-label"))).toEqual([
      "Reset two-step sign-in for user2@demo.example.org",
      "Reset two-step sign-in for user3@demo.example.org",
    ]);
  });

  it("an owner may reset an owner, never themselves", () => {
    table("owner");
    expect(screen.getByRole("button", { name: "Reset two-step sign-in for user1@demo.example.org" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Reset two-step sign-in for user9@demo.example.org" })).toBeNull();
  });

  it("no action passed (role without user.totp_reset): no button at all", () => {
    render(<UsersTable users={[user(3)]} selfId="u9" canManage actor="admin" roles={[]} update={vi.fn()} />);
    expect(screen.queryByRole("button", { name: /Reset two-step/ })).toBeNull();
  });
});

describe("TotpResetDialog (UI-3)", () => {
  it("says the user must enrol again, and sends password, code and reason once without keeping the secrets", async () => {
    const ue = userEvent.setup();
    let resolve!: (r: TotpResetResult) => void;
    const reset = vi.fn(() => new Promise<TotpResetResult>((r) => (resolve = r)));
    const onDone = vi.fn();
    render(<TotpResetDialog user={user(3)} reset={reset} onDone={onDone} onClose={vi.fn()} />);
    const dialog = screen.getByRole("dialog", { name: "Reset two-step sign-in for user3@demo.example.org" });
    expect(dialog).toHaveTextContent("They will have to set up two-step sign-in again the next time they sign in.");
    await ue.type(screen.getByLabelText("Why are you resetting it?"), "Lost phone, confirmed in person");
    await ue.type(screen.getByLabelText("Your password"), "s3cret-pass");
    await ue.type(screen.getByLabelText("Your authenticator code (if you use one)"), "123456");
    await ue.click(within(dialog).getByRole("button", { name: "Reset two-step sign-in" }));
    expect(reset).toHaveBeenCalledTimes(1);
    const [id, fd] = reset.mock.calls[0]! as unknown as [string, FormData];
    expect(id).toBe("u3");
    expect(fd.get("password")).toBe("s3cret-pass");
    expect(fd.get("totp_code")).toBe("123456");
    expect(fd.get("reason")).toBe("Lost phone, confirmed in person");
    // Cleared before the answer arrives: the secret is no longer in the DOM (and never was in React state).
    expect(screen.getByLabelText("Your password")).toHaveValue("");
    expect(screen.getByLabelText("Your authenticator code (if you use one)")).toHaveValue("");
    expect(document.body.innerHTML).not.toContain("s3cret-pass");
    resolve({ ok: true });
    await vi.waitFor(() => expect(onDone).toHaveBeenCalled());
  });

  it.each([
    ["totp-reset-forbidden", 403, /own account or a higher role/],
    ["reauthentication-failed", 401, /password or authenticator code was not accepted/],
    [null, 429, /Too many attempts/],
    ["validation-failed", 400, /reason of 10 to 200/],
  ] as const)("maps %s (%i)", async (slug, status, text) => {
    const ue = userEvent.setup();
    render(<TotpResetDialog user={user(3)} reset={vi.fn().mockResolvedValue(fail(slug, status))} onDone={vi.fn()} onClose={vi.fn()} />);
    await ue.type(screen.getByLabelText("Why are you resetting it?"), "Lost phone again");
    await ue.type(screen.getByLabelText("Your password"), "pw");
    await ue.click(screen.getByRole("button", { name: "Reset two-step sign-in" }));
    expect(await screen.findByRole("alert")).toHaveTextContent(text);
    // The reason stays so the user can retry with the right password.
    expect(screen.getByLabelText("Why are you resetting it?")).toHaveValue("Lost phone again");
  });

  it("error text falls back by status", () => {
    expect(totpResetErrorText({ ok: false, slug: null, status: 0 }, adminMessages.en.users.totpReset)).toMatch(/unavailable/);
    expect(totpResetErrorText({ ok: false, slug: null, status: 404 }, adminMessages.en.users.totpReset)).toMatch(/not found/);
  });

  it("from the table: the success line names the user", async () => {
    const ue = userEvent.setup();
    render(
      <UsersTable users={[user(3)]} selfId="u9" canManage actor="admin" roles={assignableRoles("admin")} update={vi.fn()} resetTotp={vi.fn().mockResolvedValue({ ok: true })} />,
    );
    await ue.click(screen.getByRole("button", { name: "Reset two-step sign-in for user3@demo.example.org" }));
    await ue.type(screen.getByLabelText("Why are you resetting it?"), "New phone, old one wiped");
    await ue.type(screen.getByLabelText("Your password"), "pw");
    await ue.click(within(screen.getByRole("dialog")).getByRole("button", { name: "Reset two-step sign-in" }));
    expect(await screen.findByRole("status")).toHaveTextContent("Two-step sign-in was reset for user3@demo.example.org");
    expect(screen.queryByRole("dialog")).toBeNull();
  });
});

describe("API", () => {
  it("live: POST /orgs/{id}/users/{user_id}/totp/reset with the step-up body, 204", async () => {
    const f = vi.fn().mockResolvedValue(new Response(null, { status: 204 }));
    const api = createLiveClient({ baseUrl: "https://api.test", accessToken: "tok", fetchImpl: f as unknown as typeof fetch });
    await api.resetUserTotp("o", "u/3", { password: "pw", reason: "Lost phone, confirmed" });
    const [url, init] = f.mock.calls[0]! as [string, RequestInit];
    expect(url).toBe("https://api.test/v1/orgs/o/users/u%2F3/totp/reset");
    expect(init.method).toBe("POST");
    expect(JSON.parse(String(init.body))).toEqual({ password: "pw", reason: "Lost phone, confirmed" });
  });

  it("mock: the step-up password is the mock login password", () => {
    expect(MOCK_STEP_UP_PASSWORD).toBe(MOCK_PASSWORD);
  });

  it("mock: self and higher rank 403, wrong password 401 (audited), short reason 400, success audited with the reason only", async () => {
    const api = createMockClient();
    const users = (await api.listUsers(MOCK_ORG_ID, { limit: 100 })).items;
    const me = users.find((u) => u.email === "admin@demo.example.org")!;
    const owner = users.find((u) => u.role === "owner")!;
    const clinician = users.find((u) => u.role === "clinician")!;
    const step = { password: MOCK_PASSWORD, reason: "Lost phone, confirmed in person" };
    await expect(api.resetUserTotp(MOCK_ORG_ID, me.id, step)).rejects.toMatchObject({ status: 403, slug: "totp-reset-forbidden" });
    await expect(api.resetUserTotp(MOCK_ORG_ID, owner.id, step)).rejects.toMatchObject({ status: 403, slug: "totp-reset-forbidden" });
    await expect(api.resetUserTotp(MOCK_ORG_ID, clinician.id, { ...step, reason: "short" })).rejects.toMatchObject({ status: 400 });
    await expect(api.resetUserTotp(MOCK_ORG_ID, clinician.id, { ...step, password: "nope" })).rejects.toMatchObject({
      status: 401,
      slug: "reauthentication-failed",
    });
    await api.resetUserTotp(MOCK_ORG_ID, clinician.id, step);
    const audit = (await api.listAudit(MOCK_ORG_ID, { action: "user.totp_reset", limit: 5 })).items;
    expect(audit.map((a) => a.action)).toEqual(["user.totp_reset_by_admin", "user.totp_reset_failed"]);
    expect(audit[0]).toMatchObject({ target_id: clinician.id, after: { reason: step.reason } });
    expect(JSON.stringify(audit)).not.toContain(MOCK_PASSWORD);
    await expect(api.resetUserTotp(MOCK_ORG_ID, "nobody", step)).rejects.toMatchObject({ status: 404 });
  });
});
