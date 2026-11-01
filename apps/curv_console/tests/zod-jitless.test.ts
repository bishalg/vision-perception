/**
 * CSP: zod must never reach `Function` (its JIT compiler or its `allowsEval` probe) in the browser, because the
 * production CSP has no `'unsafe-eval'` and the probe alone is a reported violation. `@/lib/zod` sets `jitless`
 * before any schema exists; this test loads the schema modules the client bundles use with a recording `Function`
 * and parses through them.
 */
import { afterAll, beforeAll, describe, expect, it, vi } from "vitest";

const calls: string[] = [];
const RealFunction = globalThis.Function;

beforeAll(() => {
  globalThis.Function = new Proxy(RealFunction, {
    apply(target, thisArg, args) {
      calls.push(`Function(${args.length} args)`);
      return Reflect.apply(target, thisArg, args);
    },
    construct(target, args, newTarget) {
      calls.push(`new Function(${args.length} args)`);
      return Reflect.construct(target, args, newTarget === globalThis.Function ? target : newTarget);
    },
  });
  vi.resetModules();
});

afterAll(() => {
  globalThis.Function = RealFunction;
});

describe("@/lib/zod", () => {
  it("sets jitless before any schema is built, and schema construction and parsing never call Function", async () => {
    const { z } = await import("@/lib/zod");
    const api = await import("@/lib/api");
    const { parseCatalog } = await import("@/lib/catalog");
    const { parseProfile, parseBaselines } = await import("@/lib/configDocs");
    await import("@/lib/asd/ops");

    // Object schemas are what zod would JIT-compile; parse valid and invalid input through several.
    api.tokenPairSchema.parse({
      access_token: "a",
      refresh_token: "r",
      expires_in: 60,
      user: { id: "u", email: "e@x", role: "admin", org_id: null },
    });
    expect(api.tokenPairSchema.safeParse({ access_token: "" }).success).toBe(false);
    expect(api.problemSchema.safeParse({ type: "t", title: "x", status: 400 }).success).toBe(true);
    expect(parseCatalog({ nope: true })).toBeNull();
    expect(parseProfile({ nope: true })).toBeNull();
    expect(parseBaselines({ nope: true })).toBeNull();
    const ad = z.object({ a: z.string(), b: z.number().optional() });
    expect(ad.parse({ a: "x" })).toEqual({ a: "x" });

    expect(calls).toEqual([]);
    expect(z.config().jitless).toBe(true);
  });
});
