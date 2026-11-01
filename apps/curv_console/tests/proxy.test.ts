/** CSP-1 / CSP-2 on the proxy: redirects carry the policy (and HSTS in production); the refresh branch forwards the
 * rotated cookies and the CSP / x-nonce request headers. */
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { NextRequest } from "next/server";
import { ACCESS_COOKIE, REFRESH_COOKIE } from "@/lib/auth/cookies";

const refresh = vi.fn();
vi.mock("@/lib/api", () => ({ getApi: () => ({ refresh }) }));

const { proxy } = await import("@/proxy");

const nonceOf = (csp: string | null) => /'nonce-([^']+)'/.exec(csp ?? "")?.[1];

function req(path: string, cookies: Record<string, string> = {}): NextRequest {
  const cookie = Object.entries(cookies).map(([k, v]) => `${k}=${v}`).join("; ");
  return new NextRequest(new URL(path, "http://console.test"), { headers: cookie ? { cookie } : {} });
}

beforeEach(() => {
  refresh.mockReset();
});
afterEach(() => vi.unstubAllEnvs());

describe("proxy redirects", () => {
  it("redirect-login carries the CSP, and HSTS only in production", async () => {
    vi.stubEnv("NODE_ENV", "test");
    const res = await proxy(req("/abc/overview"));
    expect(res.status).toBe(307);
    expect(res.headers.get("location")).toBe("http://console.test/login?next=%2Fabc%2Foverview");
    expect(res.headers.get("content-security-policy")).toMatch(/script-src 'self' 'nonce-[^']+' 'strict-dynamic'/);
    expect(res.headers.get("strict-transport-security")).toBeNull();

    vi.stubEnv("NODE_ENV", "production");
    const prod = await proxy(req("/abc/overview"));
    expect(prod.headers.get("content-security-policy")).not.toContain("'unsafe-eval'");
    expect(prod.headers.get("strict-transport-security")).toBe("max-age=63072000; includeSubDomains");
  });

  it("redirect-home carries CSP and HSTS in production", async () => {
    vi.stubEnv("NODE_ENV", "production");
    const home = await proxy(req("/login", { [ACCESS_COOKIE]: "a" }));
    expect(home.headers.get("location")).toBe("http://console.test/");
    expect(nonceOf(home.headers.get("content-security-policy"))).toBeTruthy();
    expect(home.headers.get("strict-transport-security")).toBe("max-age=63072000; includeSubDomains");

  });

  it("the failed-refresh redirect carries CSP and HSTS in production", async () => {
    vi.stubEnv("NODE_ENV", "production");
    refresh.mockRejectedValue(new Error("expired"));
    const failed = await proxy(req("/abc/overview", { [REFRESH_COOKIE]: "r" }));
    expect(failed.headers.get("location")).toBe("http://console.test/login");
    expect(nonceOf(failed.headers.get("content-security-policy"))).toBeTruthy();
    expect(failed.headers.get("strict-transport-security")).toBe("max-age=63072000; includeSubDomains");
  });
});

describe("proxy refresh", () => {
  it("forwards the rotated cookies and the CSP / x-nonce request headers", async () => {
    refresh.mockResolvedValue({
      access_token: "new-access",
      refresh_token: "new-refresh",
      expires_in: 900,
      user: { id: "u", email: "e@x", role: "admin", org_id: "org-1" },
    });
    const res = await proxy(req("/abc/overview", { [REFRESH_COOKIE]: "old-refresh" }));
    expect(refresh).toHaveBeenCalledWith("old-refresh");
    // Browser side: Set-Cookie with the rotated pair.
    expect(res.cookies.get(ACCESS_COOKIE)?.value).toBe("new-access");
    expect(res.cookies.get(REFRESH_COOKIE)?.value).toBe("new-refresh");
    // Server-component side: the overridden request headers include the rotated cookies, the policy and the nonce.
    const overridden = res.headers.get("x-middleware-override-headers")?.split(",") ?? [];
    expect(overridden).toEqual(expect.arrayContaining(["cookie", "content-security-policy", "x-nonce"]));
    const cookie = res.headers.get("x-middleware-request-cookie") ?? "";
    expect(cookie).toContain(`${ACCESS_COOKIE}=new-access`);
    expect(cookie).toContain(`${REFRESH_COOKIE}=new-refresh`);
    const csp = res.headers.get("content-security-policy");
    expect(res.headers.get("x-middleware-request-content-security-policy")).toBe(csp);
    expect(res.headers.get("x-middleware-request-x-nonce")).toBe(nonceOf(csp));
  });
});
