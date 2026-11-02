import { describe, expect, it } from "vitest";
import { decideGuard, enforcementRedirect, isAnonymousPath, isPublicPath, safeNextPath } from "@/lib/auth/guard";
import { ACCESS_COOKIE, ORG_COOKIE, REFRESH_COOKIE, ROLE_COOKIE, SESSION_COOKIE_NAMES, readRole, sessionCookies } from "@/lib/auth/cookies";

describe("guard", () => {
  it("lets unauthenticated users reach /login only", () => {
    expect(decideGuard("/login", false, false)).toBe("allow");
    expect(decideGuard("/org/overview", false, false)).toBe("redirect-login");
  });
  it("refreshes when only the refresh cookie remains", () => {
    expect(decideGuard("/org/overview", false, true)).toBe("refresh");
  });
  it("allows authenticated users and bounces them off /login", () => {
    expect(decideGuard("/org/overview", true, true)).toBe("allow");
    expect(decideGuard("/login", true, true)).toBe("redirect-home");
  });
});

describe("guard: anonymous share paths (WP-C.19, SHR-9)", () => {
  const paths = ["/share", "/share/5a5e0000-0000-4000-8000-000000000001", "/api/share/x", "/share/x?e=gone"];
  it.each(paths)("%s is allowed with or without a session, never redirect-home or refresh", (path) => {
    const p = path.split("?")[0]!;
    for (const [access, refresh] of [[false, false], [true, true], [true, false], [false, true]] as const) {
      expect(decideGuard(p, access, refresh)).toBe("allow");
    }
    expect(isAnonymousPath(p)).toBe(true);
    // Public for proxy.ts: no HIP-2 verdict, no `curv_last_seen` refresh.
    expect(isPublicPath(p)).toBe(true);
  });
  it.each(["/shared", "/sharex/y", "/api/shares", "/api/export/x", "/o/share/x", "/loginx"])("%s is not anonymous", (p) => {
    expect(isAnonymousPath(p)).toBe(false);
  });
  it("the sign-in page keeps its rules", () => {
    expect(isAnonymousPath("/login")).toBe(false);
    expect(isPublicPath("/login")).toBe(true);
    expect(decideGuard("/login", true, true)).toBe("redirect-home");
    expect(decideGuard("/api/export/x", false, false)).toBe("redirect-login");
  });
});

describe("safeNextPath", () => {
  it.each([["//evil.com"], ["https://evil.com"], ["/\\evil"], ["/login"], [""], [null], [undefined]])(
    "rejects %s",
    (v) => expect(safeNextPath(v as string | null | undefined)).toBeNull(),
  );
  it("accepts same-site paths", () => {
    expect(safeNextPath("/o/sessions?flag=red")).toBe("/o/sessions?flag=red");
  });
});

describe("session cookies", () => {
  const pair = {
    access_token: "a", refresh_token: "r", expires_in: 900,
    user: { id: "u", email: "e@x.org", role: "admin" as const, org_id: "o" },
  };
  it("are httpOnly, lax, path=/ and the access cookie expires with the token", () => {
    const cookies = sessionCookies(pair, true);
    for (const c of cookies) {
      expect(c.options).toMatchObject({ httpOnly: true, sameSite: "lax", path: "/", secure: true });
    }
    expect(cookies.find((c) => c.name === ACCESS_COOKIE)!.options.maxAge).toBe(900);
    expect(cookies.find((c) => c.name === REFRESH_COOKIE)!.options.maxAge).toBeGreaterThan(900);
    expect(cookies.find((c) => c.name === ORG_COOKIE)!.value).toBe("o");
    expect(cookies.find((c) => c.name === ROLE_COOKIE)!.value).toBe("admin");
  });
  it("the role cookie is read back only when it is a contract role, and logout clears it", () => {
    expect(readRole("reviewer")).toBe("reviewer");
    expect(readRole("god")).toBeUndefined();
    expect(readRole(undefined)).toBeUndefined();
    expect(SESSION_COOKIE_NAMES).toContain(ROLE_COOKIE);
  });
  it("set no org cookie for the superadmin, whose org_id is null", () => {
    const root = { ...pair, user: { ...pair.user, role: "superadmin" as const, org_id: null } };
    const names = sessionCookies(root, true).map((c) => c.name);
    expect(names).toEqual([ACCESS_COOKIE, REFRESH_COOKIE, ROLE_COOKIE]);
  });
});

describe("enforcementRedirect (WP-C.17, AUTH-13)", () => {
  const org = "o";
  const redirectTo = "/o/settings/security?required=1";
  const cases: [string, "not_required" | "grace" | "required" | undefined, string | null, string | null][] = [
    ["not_required never redirects", "not_required", "/o/overview", null],
    ["grace never redirects", "grace", "/o/overview", null],
    ["unknown profile (older backend or failed read) never redirects", undefined, "/o/overview", null],
    ["required on overview", "required", "/o/overview", redirectTo],
    ["required on sessions", "required", "/o/sessions", redirectTo],
    ["required on the security page: no loop", "required", "/o/settings/security", null],
    ["required below the security page", "required", "/o/settings/security/", null],
    ["required with no pathname header goes to security", "required", null, redirectTo],
    ["a look-alike path is not the security page", "required", "/o/settings/security-x", redirectTo],
    ["another org's security page is not exempt", "required", "/p/settings/security", redirectTo],
  ];
  for (const [name, enforcement, pathname, expected] of cases) {
    it(name, () => {
      expect(enforcementRedirect(enforcement === undefined ? null : { totp_enforcement: enforcement }, pathname, org)).toBe(expected);
    });
  }

  it("encodes the org id", () => {
    expect(enforcementRedirect({ totp_enforcement: "required" }, "/a%20b/overview", "a b")).toBe(
      "/a%20b/settings/security?required=1",
    );
  });
});
