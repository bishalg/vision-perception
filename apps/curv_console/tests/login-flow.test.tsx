import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { ApiError, PROBLEM_BASE } from "@/lib/api";

const { login, writeSession, redirect } = vi.hoisted(() => ({
  login: vi.fn(),
  writeSession: vi.fn(),
  redirect: vi.fn((to: string) => {
    throw new Error(`REDIRECT:${to}`);
  }),
}));
vi.mock("next/navigation", () => ({ redirect: (to: string) => redirect(to) }));
vi.mock("@/lib/api", async (orig) => ({ ...(await orig<typeof import("@/lib/api")>()), getApi: () => ({ login }) }));
vi.mock("@/lib/auth/server", () => ({ writeSession, clearSession: vi.fn(), readRefreshToken: vi.fn() }));

import { loginAction } from "@/app/login/actions";
import { LoginForm } from "@/app/login/LoginForm";

const problem = (status: number, slug: string, retryAfter: number | null = null) =>
  new ApiError(status, { type: `${PROBLEM_BASE}${slug}`, title: slug, status }, undefined, retryAfter);
const pair = { access_token: "a", refresh_token: "r", expires_in: 900, user: { id: "u", email: "e@x.org", role: "admin", org_id: "org1" } };
const fd = (o: Record<string, string>) => {
  const f = new FormData();
  for (const [k, v] of Object.entries(o)) f.set(k, v);
  return f;
};
const idle = { error: null, step: "password" } as const;

beforeEach(() => {
  login.mockReset();
  writeSession.mockReset();
  redirect.mockClear();
});

describe("loginAction", () => {
  it("moves to the TOTP step on totp-required", async () => {
    login.mockRejectedValueOnce(problem(401, "totp-required"));
    expect(await loginAction(idle, fd({ email: "e@x.org", password: "pw" }))).toEqual({ error: null, step: "totp" });
    expect(login).toHaveBeenCalledWith("e@x.org", "pw", undefined);
  });

  it("sends email, password and code together, then signs in", async () => {
    login.mockResolvedValueOnce(pair);
    await expect(loginAction(idle, fd({ email: "e@x.org", password: "pw", totp_code: "123 456" }))).rejects.toThrow("REDIRECT:/org1/overview");
    expect(login).toHaveBeenCalledWith("e@x.org", "pw", "123456");
    expect(writeSession).toHaveBeenCalledWith(pair);
  });

  it("stays on the TOTP step on totp-invalid, and rejects malformed codes locally", async () => {
    login.mockRejectedValueOnce(problem(401, "totp-invalid"));
    expect(await loginAction(idle, fd({ email: "e@x.org", password: "pw", totp_code: "000000" }))).toEqual({ error: "totp-invalid", step: "totp" });
    expect(await loginAction(idle, fd({ email: "e@x.org", password: "pw", totp_code: "12ab" }))).toEqual({ error: "totp-invalid", step: "totp" });
    expect(login).toHaveBeenCalledTimes(1);
  });

  it("maps wrong password, 429 and 503", async () => {
    login.mockRejectedValueOnce(problem(401, "unauthorized"));
    expect(await loginAction(idle, fd({ email: "e@x.org", password: "x" }))).toMatchObject({ error: "failed", step: "password" });
    login.mockRejectedValueOnce(problem(429, "rate-limited", 42));
    expect(await loginAction(idle, fd({ email: "e@x.org", password: "x" }))).toMatchObject({ error: "rate-limited", retryAfter: 42 });
    login.mockRejectedValueOnce(problem(503, "clinical-crypto-unavailable"));
    expect(await loginAction(idle, fd({ email: "e@x.org", password: "x", totp_code: "123456" }))).toMatchObject({ error: "server-problem", step: "totp" });
  });
});

describe("LoginForm TOTP step", () => {
  it("shows the code field after the password step and resubmits the password with it", async () => {
    const action = vi
      .fn()
      .mockResolvedValueOnce({ error: null, step: "totp" })
      .mockResolvedValueOnce({ error: "totp-invalid", step: "totp" });
    render(<LoginForm action={action} />);
    const user = userEvent.setup();
    await user.type(screen.getByLabelText("Email"), "e@x.org");
    await user.type(screen.getByLabelText("Password"), "pw");
    await user.click(screen.getByRole("button", { name: "Sign in" }));
    const code = await screen.findByLabelText("Authentication code");
    expect(code).toHaveAttribute("inputmode", "numeric");
    expect(code).toHaveAttribute("autocomplete", "one-time-code");
    await user.type(code, "000000");
    await user.click(screen.getByRole("button", { name: "Sign in" }));
    await waitFor(() => expect(action).toHaveBeenCalledTimes(2));
    const sent = action.mock.calls[1]![1] as FormData;
    expect([sent.get("email"), sent.get("password"), sent.get("totp_code")]).toEqual(["e@x.org", "pw", "000000"]);
    expect(await screen.findByText("That code is wrong or has expired. Try again.")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Use a different account" }));
    expect(await screen.findByLabelText("Password")).toHaveValue("");
  });

  it("shows the retry-after message", async () => {
    const action = vi.fn().mockResolvedValue({ error: "rate-limited", step: "password", retryAfter: 30 });
    render(<LoginForm action={action} />);
    const user = userEvent.setup();
    await user.type(screen.getByLabelText("Email"), "e@x.org");
    await user.type(screen.getByLabelText("Password"), "pw");
    await user.click(screen.getByRole("button", { name: "Sign in" }));
    expect(await screen.findByText("Too many attempts. Try again in 30 seconds.")).toBeInTheDocument();
  });
});
