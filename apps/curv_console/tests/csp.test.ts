/** CSP-1 / CSP-2: the policy builder and the static security headers. */
import { afterEach, describe, expect, it, vi } from "vitest";
import config from "../next.config";
import { buildCsp, newNonce } from "@/lib/csp";

const directives = (csp: string) => Object.fromEntries(csp.split("; ").map((d) => [d.split(" ")[0], d.split(" ").slice(1)]));

describe("newNonce", () => {
  it("is 16 random bytes, base64, different per call", () => {
    const a = newNonce();
    const b = newNonce();
    expect(a).not.toBe(b);
    expect(Buffer.from(a, "base64")).toHaveLength(16);
  });
});

describe("buildCsp", () => {
  it("carries the nonce and the spec directives in production", () => {
    const d = directives(buildCsp("abc==", false));
    expect(d["script-src"]).toEqual(["'self'", "'nonce-abc=='", "'strict-dynamic'", "'wasm-unsafe-eval'"]);
    expect(d["default-src"]).toEqual(["'self'"]);
    expect(d["style-src"]).toEqual(["'self'", "'unsafe-inline'"]);
    expect(d["img-src"]).toEqual(["'self'", "blob:"]);
    expect(d["font-src"]).toEqual(["'self'"]);
    expect(d["connect-src"]).toEqual(["'self'"]);
    expect(d["object-src"]).toEqual(["'none'"]);
    expect(d["base-uri"]).toEqual(["'self'"]);
    expect(d["form-action"]).toEqual(["'self'"]);
    expect(d["frame-ancestors"]).toEqual(["'none'"]);
  });

  it("allows unsafe-eval only in development and never unsafe-inline in script-src", () => {
    expect(buildCsp("n", false)).not.toContain("'unsafe-eval'");
    const dev = directives(buildCsp("n", true));
    expect(dev["script-src"]).toContain("'unsafe-eval'");
    for (const dev of [true, false]) expect(directives(buildCsp("n", dev))["script-src"]).not.toContain("'unsafe-inline'");
  });

  it("differs per nonce", () => {
    expect(buildCsp(newNonce(), false)).not.toBe(buildCsp(newNonce(), false));
  });
});

describe("next.config headers (CSP-2)", () => {
  afterEach(() => vi.unstubAllEnvs());
  const headersFor = async (nodeEnv: string) => {
    vi.stubEnv("NODE_ENV", nodeEnv);
    const rules = await config.headers!();
    const all = rules.find((r) => r.source === "/:path*")!;
    return Object.fromEntries(all.headers.map((x) => [x.key, x.value]));
  };

  it("sets HSTS in production only", async () => {
    expect((await headersFor("production"))["Strict-Transport-Security"]).toBe("max-age=63072000; includeSubDomains");
    expect((await headersFor("development"))["Strict-Transport-Security"]).toBeUndefined();
    expect((await headersFor("test"))["Strict-Transport-Security"]).toBeUndefined();
  });

  it("keeps the other security headers on every path", async () => {
    const h = await headersFor("production");
    expect(h["X-Frame-Options"]).toBe("DENY");
    expect(h["X-Content-Type-Options"]).toBe("nosniff");
    expect(h["Referrer-Policy"]).toBe("no-referrer");
    expect(h["Content-Security-Policy"]).toBeUndefined();
  });
});
