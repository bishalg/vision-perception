/** CSP-1 / CSP-2 on the proxy: redirects carry the policy (and HSTS in production); the refresh branch forwards the
 * rotated cookies and the CSP / x-nonce request headers. */
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { NextRequest } from "next/server";
import { ACCESS_COOKIE, LAST_SEEN_COOKIE, LOGIN_AT_COOKIE, REFRESH_COOKIE } from "@/lib/auth/cookies";

const refresh = vi.fn();
vi.mock("@/lib/api", () => ({ getApi: () => ({ refresh }) }));

const { proxy } = await import("@/proxy");

const nonceOf = (csp: string | null) => /'nonce-([^']+)'/.exec(csp ?? "")?.[1];

function req(path: string, cookies: Record<string, string> = {}, extra: Record<string, string> = {}): NextRequest {
  const cookie = Object.entries(cookies).map(([k, v]) => `${k}=${v}`).join("; ");
  return new NextRequest(new URL(path, "http://console.test"), { headers: { ...(cookie ? { cookie } : {}), ...extra } });
}

/** HIP-2 stamps of a session within policy (unstamped sessions are `expired` and never reach the refresh branch). */
const fresh = () => ({ [LAST_SEEN_COOKIE]: String(Date.now() - 60_000), [LOGIN_AT_COOKIE]: String(Date.now() - 3_600_000) });

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
    const failed = await proxy(req("/abc/overview", { [REFRESH_COOKIE]: "r", ...fresh() }));
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
    const res = await proxy(req("/abc/overview", { [REFRESH_COOKIE]: "old-refresh", ...fresh() }));
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

  it("forwards the request pathname as x-pathname (WP-C.17: the org layout's enforcement redirect reads it)", async () => {
    refresh.mockResolvedValue({
      access_token: "a", refresh_token: "r", expires_in: 900,
      user: { id: "u", email: "e@x", role: "admin", org_id: "org-1" },
    });
    const res = await proxy(req("/org-1/sessions", { [REFRESH_COOKIE]: "old-refresh", ...fresh() }));
    expect(res.headers.get("x-middleware-override-headers")?.split(",")).toContain("x-pathname");
    expect(res.headers.get("x-middleware-request-x-pathname")).toBe("/org-1/sessions");
    // The plain pass-through (valid access cookie) forwards it too.
    const plain = await proxy(req("/org-1/overview", { [ACCESS_COOKIE]: "tok", ...fresh() }));
    expect(plain.headers.get("x-middleware-request-x-pathname")).toBe("/org-1/overview");
  });
});

describe("automatic logoff (WP-C.21, HIP-2)", () => {
  const MIN = 60_000;
  const HOUR = 60 * MIN;
  const doc = { "sec-fetch-dest": "document" };
  const session = (lastSeenAgo: number, loginAgo: number) => ({
    [ACCESS_COOKIE]: "a",
    [REFRESH_COOKIE]: "r",
    [LAST_SEEN_COOKIE]: String(Date.now() - lastSeenAgo),
    [LOGIN_AT_COOKIE]: String(Date.now() - loginAgo),
  });

  it("fresh cookies pass and a document load refreshes curv_last_seen, not curv_login_at", async () => {
    const res = await proxy(req("/abc/overview", session(5 * MIN, HOUR), doc));
    expect(res.status).toBe(200);
    expect(res.headers.get("location")).toBeNull();
    const seen = Number(res.cookies.get(LAST_SEEN_COOKIE)?.value);
    expect(Date.now() - seen).toBeLessThan(5_000);
    expect(res.cookies.get(LOGIN_AT_COOKIE)).toBeUndefined();
  });

  it("idle past 30 minutes redirects to /login?reason=idle, clears auth cookies, keeps the CSP", async () => {
    const res = await proxy(req("/abc/overview", session(31 * MIN, HOUR), doc));
    expect(res.status).toBe(307);
    const loc = new URL(res.headers.get("location") ?? "");
    expect(loc.pathname).toBe("/login");
    expect(loc.searchParams.get("reason")).toBe("idle");
    expect(loc.searchParams.get("next")).toBe("/abc/overview");
    for (const name of [ACCESS_COOKIE, REFRESH_COOKIE, LAST_SEEN_COOKIE, LOGIN_AT_COOKIE, "curv_role", "curv_org"]) {
      expect(res.cookies.get(name)?.value, name).toBe("");
    }
    expect(nonceOf(res.headers.get("content-security-policy"))).toBeTruthy();
  });

  it("login older than 12 hours redirects with reason=expired even when recently active", async () => {
    const res = await proxy(req("/abc/overview", session(MIN, 13 * HOUR), doc));
    expect(new URL(res.headers.get("location") ?? "").searchParams.get("reason")).toBe("expired");
  });

  it("a session without stamps (pre-policy) is expired; the refresh branch is not reached", async () => {
    const res = await proxy(req("/abc/overview", { [REFRESH_COOKIE]: "r" }, doc));
    expect(new URL(res.headers.get("location") ?? "").searchParams.get("reason")).toBe("expired");
    expect(refresh).not.toHaveBeenCalled();
  });

  it("RSC navigations and server actions refresh curv_last_seen within policy", async () => {
    const fresh = session(10 * MIN, HOUR);
    const variants: Record<string, string>[] = [
      { rsc: "1", "sec-fetch-dest": "empty" },
      { "sec-fetch-dest": "empty", "next-action": "abc" },
    ];
    for (const headers of variants) {
      const res = await proxy(req("/abc/overview", fresh, headers));
      expect(res.status).toBe(200);
      expect(Date.now() - Number(res.cookies.get(LAST_SEEN_COOKIE)?.value)).toBeLessThan(5_000);
      expect(res.cookies.get(LOGIN_AT_COOKIE)).toBeUndefined();
    }
  });

  const expectCleared = (res: Response & { cookies: { get(name: string): { value: string } | undefined } }) => {
    for (const name of [ACCESS_COOKIE, REFRESH_COOKIE, LAST_SEEN_COOKIE, LOGIN_AT_COOKIE, "curv_role", "curv_org"]) {
      expect(res.cookies.get(name)?.value, name).toBe("");
    }
  };

  it("an idle RSC navigation is redirected to /login?reason=idle, cookies cleared, no token minted (defect 1)", async () => {
    // Access cookie gone, refresh cookie present: the guard would mint new tokens if the verdict did not stop it.
    const { [ACCESS_COOKIE]: _drop, ...noAccess } = session(31 * MIN, HOUR);
    void _drop;
    // Next strips `RSC` and `_rsc` before the proxy runs, so an RSC navigation looks like a plain same-origin fetch.
    const res = await proxy(req("/abc/sessions?flag=red", noAccess, { "sec-fetch-dest": "empty", "sec-fetch-mode": "cors" }));
    expect(res.status).toBe(307);
    const loc = new URL(res.headers.get("location") ?? "");
    expect(loc.pathname).toBe("/login");
    expect(loc.searchParams.get("reason")).toBe("idle");
    expect(loc.searchParams.get("next")).toBe("/abc/sessions?flag=red");
    expect(refresh).not.toHaveBeenCalled();
    expectCleared(res);
    expect(res.cookies.get(ACCESS_COOKIE)?.value).toBe("");
    expect(nonceOf(res.headers.get("content-security-policy"))).toBeTruthy();
    expect(res.cookies.get("curv_logoff_reason")?.value).toBe("idle");
  });

  it("a request after the session ended keeps the reason on its login redirect, once", async () => {
    const res = await proxy(req("/abc/overview", { curv_logoff_reason: "idle" }, { "sec-fetch-dest": "empty" }));
    expect(res.status).toBe(307);
    const loc = new URL(res.headers.get("location") ?? "");
    expect(loc.searchParams.get("reason")).toBe("idle");
    expect(loc.searchParams.get("next")).toBe("/abc/overview");
    expect(res.cookies.get("curv_logoff_reason")?.value).toBe("");
    // Without it (never signed in, or signed out by hand) there is no reason.
    const plain = await proxy(req("/abc/overview"));
    expect(new URL(plain.headers.get("location") ?? "").searchParams.get("reason")).toBeNull();
    // A forged value is ignored.
    const forged = await proxy(req("/abc/overview", { curv_logoff_reason: "<x>" }));
    expect(new URL(forged.headers.get("location") ?? "").searchParams.get("reason")).toBeNull();
  });

  it("an expired server action gets Next's action redirect to /login?reason=expired instead of running", async () => {
    const res = await proxy(
      req("/abc/settings/security", session(MIN, 13 * HOUR), { "sec-fetch-dest": "empty", "next-action": "abc" }),
    );
    expect(res.status).toBe(200);
    expect(res.headers.get("x-middleware-next")).toBeNull();
    const [target, kind] = (res.headers.get("x-action-redirect") ?? "").split(";");
    expect(kind).toBe("replace");
    const loc = new URL(target ?? "", "http://console.test");
    expect(loc.pathname).toBe("/login");
    expect(loc.searchParams.get("reason")).toBe("expired");
    expect(loc.searchParams.get("next")).toBe("/abc/settings/security");
    expect(refresh).not.toHaveBeenCalled();
    expectCleared(res);
  });

  it("an idle plain fetch of a route handler is a 401 and clears the cookies", async () => {
    const res = await proxy(req("/api/report/S-1", session(31 * MIN, HOUR), { "sec-fetch-dest": "empty" }));
    expect(res.status).toBe(401);
    expect(res.headers.get("x-middleware-next")).toBeNull();
    expect((await res.json()).login).toMatch(/^\/login\?reason=idle/);
    expectCleared(res);
  });

  it("prefetch requests do not refresh curv_last_seen", async () => {
    const stale = session(10 * MIN, HOUR);
    const variants: Record<string, string>[] = [
      { rsc: "1", "next-router-prefetch": "1", "sec-fetch-dest": "empty" },
      { purpose: "prefetch", "sec-fetch-dest": "empty" },
    ];
    for (const headers of variants) {
      const res = await proxy(req("/abc/overview", stale, headers));
      expect(res.status).toBe(200);
      expect(res.cookies.get(LAST_SEEN_COOKIE)).toBeUndefined();
    }
  });

  it("an idle prefetch renders nothing, refreshes nothing and keeps the cookies for the navigation's reason", async () => {
    const { [ACCESS_COOKIE]: _drop, ...noAccess } = session(31 * MIN, HOUR);
    void _drop;
    const res = await proxy(req("/abc/overview", noAccess, { rsc: "1", "next-router-prefetch": "1", "sec-fetch-dest": "empty" }));
    expect(res.status).toBe(204);
    expect(res.headers.get("x-middleware-next")).toBeNull();
    expect(refresh).not.toHaveBeenCalled();
    expect(res.cookies.getAll()).toEqual([]);
  });

  it("the public /login page is never redirected by the policy", async () => {
    const res = await proxy(req("/login", {}, doc));
    expect(res.status).toBe(200);
  });

  it("token rotation on a document load keeps curv_login_at (the absolute limit is not extended)", async () => {
    refresh.mockResolvedValue({
      access_token: "n", refresh_token: "nr", expires_in: 900,
      user: { id: "u", email: "e@x", role: "admin", org_id: "org-1" },
    });
    const res = await proxy(req("/abc/overview", {
      [REFRESH_COOKIE]: "r",
      [LAST_SEEN_COOKIE]: String(Date.now() - MIN),
      [LOGIN_AT_COOKIE]: String(Date.now() - HOUR),
    }, doc));
    expect(res.cookies.get(ACCESS_COOKIE)?.value).toBe("n");
    expect(res.cookies.get(LOGIN_AT_COOKIE)).toBeUndefined();
    expect(res.cookies.get(LAST_SEEN_COOKIE)?.value).toBeTruthy();
  });
});
