import { describe, expect, it } from "vitest";
import { decideGuard, safeNextPath } from "@/lib/auth/guard";
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
