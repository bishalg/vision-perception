/** WP-C.15 TOTP enrolment: live routes and bodies, the mock state machine, and the panel flow. */
import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { createLiveClient } from "@/lib/api/live";
import { createMockClient, MOCK_PASSWORD, MOCK_TOTP_CODE } from "@/lib/api/mock";
import { MOCK_TOTP_SECRET } from "@/lib/api/mock/totp";
import { TotpPanel, type TotpActions } from "@/components/security/TotpPanel";
import { TotpNudge } from "@/components/security/TotpNudge";
import { navLinks } from "@/lib/nav";
import { shouldNudgeTotp } from "@/lib/permissions";
import { ROLES } from "@/lib/api";

const json = (body: unknown, status = 200) =>
  new Response(JSON.stringify(body), { status, headers: { "Content-Type": "application/json" } });

describe("live TOTP client", () => {
  it("calls the three /users/me/totp routes with the contract bodies", async () => {
    const seed = { secret: "S", otpauth_url: "otpauth://totp/x", expires_at: "2026-11-01T12:10:00Z" };
    const f = vi
      .fn()
      .mockResolvedValueOnce(json(seed))
      .mockResolvedValueOnce(json({ totp_enrolled: true }))
      .mockResolvedValueOnce(json({ totp_enrolled: false }));
    const api = createLiveClient({ baseUrl: "https://api.test", accessToken: "tok", fetchImpl: f as unknown as typeof fetch });
    expect(await api.enrolTotp("pw")).toEqual(seed);
    expect(await api.confirmTotp("123456")).toEqual({ totp_enrolled: true });
    expect(await api.disableTotp("pw", "654321")).toEqual({ totp_enrolled: false });
    const calls = f.mock.calls.map(([url, init]) => [url, init.method, JSON.parse(init.body), init.headers.Authorization]);
    expect(calls).toEqual([
      ["https://api.test/v1/users/me/totp/enrol", "POST", { password: "pw" }, "Bearer tok"],
      ["https://api.test/v1/users/me/totp/confirm", "POST", { totp_code: "123456" }, "Bearer tok"],
      ["https://api.test/v1/users/me/totp", "DELETE", { password: "pw", totp_code: "654321" }, "Bearer tok"],
    ]);
  });

  it("omits totp_code on disable when none is given", async () => {
    const f = vi.fn().mockResolvedValue(json({ totp_enrolled: false }));
    const api = createLiveClient({ baseUrl: "https://api.test", fetchImpl: f as unknown as typeof fetch });
    await api.disableTotp("pw");
    expect(JSON.parse(f.mock.calls[0]![1].body)).toEqual({ password: "pw" });
  });
});

describe("mock TOTP", () => {
  it("enrol -> confirm -> already enrolled -> disable, with the backend's problem slugs", async () => {
    const api = createMockClient();
    await expect(api.enrolTotp("wrong")).rejects.toMatchObject({ status: 401, slug: "invalid-credentials" });
    await expect(api.confirmTotp(MOCK_TOTP_CODE)).rejects.toMatchObject({ status: 410, slug: "totp-enrolment-expired" });
    await expect(api.disableTotp(MOCK_PASSWORD, MOCK_TOTP_CODE)).rejects.toMatchObject({ status: 404, slug: "totp-not-enrolled" });
    const seed = await api.enrolTotp(MOCK_PASSWORD);
    expect(seed.secret).toBe(MOCK_TOTP_SECRET);
    expect(seed.otpauth_url).toMatch(/^otpauth:\/\/totp\/.+\?secret=JBSW.+&digits=6&period=30$/);
    await expect(api.confirmTotp("000000")).rejects.toMatchObject({ status: 401, slug: "totp-invalid" });
    expect(await api.confirmTotp(MOCK_TOTP_CODE)).toEqual({ totp_enrolled: true });
    await expect(api.enrolTotp(MOCK_PASSWORD)).rejects.toMatchObject({ status: 409, slug: "totp-already-enrolled" });
    await expect(api.disableTotp(MOCK_PASSWORD)).rejects.toMatchObject({ status: 401, slug: "totp-required" });
    expect(await api.disableTotp(MOCK_PASSWORD, MOCK_TOTP_CODE)).toEqual({ totp_enrolled: false });
  });

  it("GET /users/me reports totp_enrolled and totp_pending from the enrolment state", async () => {
    const api = createMockClient();
    expect(await api.getMyProfile()).toMatchObject({ totp_enrolled: false, totp_pending: false });
    await api.enrolTotp(MOCK_PASSWORD);
    expect(await api.getMyProfile()).toMatchObject({ totp_enrolled: false, totp_pending: true });
    await api.confirmTotp(MOCK_TOTP_CODE);
    expect(await api.getMyProfile()).toMatchObject({ totp_enrolled: true, totp_pending: false });
  });
});

describe("TOTP nudge", () => {
  it("nudges owner, admin, clinician and superadmin only when totp_enrolled is false", () => {
    const nudged = ROLES.filter((r) => shouldNudgeTotp(r, false));
    expect(nudged.sort()).toEqual(["admin", "clinician", "owner", "superadmin"]);
    expect(shouldNudgeTotp("admin", true)).toBe(false);
    expect(shouldNudgeTotp("admin", null)).toBe(false);
    expect(shouldNudgeTotp("admin", undefined)).toBe(false);
    expect(shouldNudgeTotp(undefined, false)).toBe(false);
  });

  it("links to the security page", () => {
    render(<TotpNudge org="o 1" role="owner" totpEnrolled={false} />);
    expect(screen.getByRole("link", { name: "Set it up" })).toHaveAttribute("href", "/o%201/settings/security");
  });
});

describe("TotpPanel", () => {
  const seed = { secret: "JBSWY3DPEHPK3PXP", otpauth_url: "otpauth://totp/Curv:me?secret=JBSWY3DPEHPK3PXP", expires_at: "2026-11-01T12:10:00Z" };

  it("shows the QR and seed once, confirms, then drops the seed", async () => {
    const actions: TotpActions = {
      enrol: vi.fn().mockResolvedValue({ ok: true, ...seed }),
      confirm: vi.fn().mockResolvedValue({ ok: true, totp_enrolled: true }),
      disable: vi.fn(),
    };
    render(<TotpPanel actions={actions} />);
    const user = userEvent.setup();
    const setup = screen.getByRole("region", { name: "Set up two-step sign-in" });
    await user.type(setup.querySelector("input[name=password]")!, "pw");
    await user.click(screen.getByRole("button", { name: "Get a key" }));
    expect(await screen.findByTestId("totp-secret")).toHaveTextContent(seed.secret);
    expect(screen.getByRole("img", { name: "QR code for your authenticator app" })).toBeInTheDocument();
    expect((actions.enrol as ReturnType<typeof vi.fn>).mock.calls[0]![0].get("password")).toBe("pw");
    await user.type(setup.querySelector("input[name=totp_code]")!, "123456");
    await user.click(screen.getByRole("button", { name: "Confirm" }));
    await vi.waitFor(() => expect(actions.confirm).toHaveBeenCalled(), { timeout: 3000 });
    expect(actions.enrol).toHaveBeenCalledTimes(1);
    expect(await screen.findByRole("status")).toHaveTextContent("Two-step sign-in is on.");
    expect(screen.queryByTestId("totp-secret")).toBeNull();
    expect(screen.queryByText(seed.secret)).toBeNull();
  });

  it("shows the current status from GET /users/me", () => {
    render(<TotpPanel actions={{ enrol: vi.fn(), confirm: vi.fn(), disable: vi.fn() }} initialEnrolled />);
    expect(screen.getByRole("status")).toHaveTextContent("Two-step sign-in is on.");
  });

  it("shows the backend reason when turning off fails", async () => {
    const actions: TotpActions = {
      enrol: vi.fn(),
      confirm: vi.fn(),
      disable: vi.fn().mockResolvedValue({ ok: false, slug: "totp-invalid", status: 401 }),
    };
    render(<TotpPanel actions={actions} />);
    const user = userEvent.setup();
    const off = screen.getByRole("region", { name: "Turn off two-step sign-in" });
    await user.type(off.querySelector("input[name=password]")!, "pw");
    await user.type(off.querySelector("input[name=totp_code]")!, "111111");
    await user.click(screen.getByRole("button", { name: "Turn off" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("That code is not correct");
  });
});

describe("nav", () => {
  it("links every role to its own account security page", () => {
    for (const role of ROLES) expect(navLinks("/o", role).some((l) => l.href === "/o/settings/security")).toBe(true);
  });
});
