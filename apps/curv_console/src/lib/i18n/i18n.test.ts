import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";
import { problemText } from "@/lib/api/problem";
import { createMockClient } from "@/lib/api/mock";
import { formatAge, formatNumber, formatPercent } from "@/lib/format";
import { messagesFor } from "./messages";
import { matchAcceptLanguage, negotiate } from "./negotiate";
import type { Locale, Widen } from "./types";

interface Row {
  user: Locale | null;
  org: Locale | null;
  acceptLanguage: string | null;
  expected: Locale;
}

const cases: Row[] = JSON.parse(
  readFileSync(resolve(process.cwd(), "../../docs/spec/schemas/asd/i18n-negotiation-cases.json"), "utf8"),
);

describe("negotiation fixture (I18N-12)", () => {
  it("has at least 20 rows", () => expect(cases.length).toBeGreaterThanOrEqual(20));
  for (const row of cases) {
    it(`${JSON.stringify(row)}`, () => {
      expect(negotiate(row.user, row.org, row.acceptLanguage)).toBe(row.expected);
    });
  }
  it("matchAcceptLanguage is null when nothing matches (the no-session fallback adds `en`)", () => {
    expect(matchAcceptLanguage("fr")).toBeNull();
    expect(matchAcceptLanguage(null)).toBeNull();
    expect(matchAcceptLanguage("ne-NP;q=0.8,en;q=0.9")).toBe("en");
  });
});

describe("Widen (I18N-15)", () => {
  const sample = { a: "x", nested: { b: "y", f: (n: number) => `n${n}` } } as const;
  type En = typeof sample;
  it("accepts any string for a literal and keeps function signatures", () => {
    expect(sample.a).toBe("x");
    const ne: Widen<En> = { a: "क", nested: { b: "ख", f: (n: number) => `${n}` } };
    expect(ne.a).toBe("क");
  });
  it("a missing key is a type error", () => {
    // @ts-expect-error `nested.b` is missing from `ne`
    const bad: Widen<En> = { a: "क", nested: { f: (n: number) => `${n}` } };
    expect(bad).toBeDefined();
  });
  it("an extra key is a type error", () => {
    // @ts-expect-error `extra` is not in `en`
    const bad: Widen<En> = { a: "क", extra: "z", nested: { b: "ख", f: (n: number) => `${n}` } };
    expect(bad).toBeDefined();
  });
});

describe("messagesFor", () => {
  it("assembles every file for both locales", () => {
    for (const locale of ["en", "ne"] as const) {
      const t = messagesFor(locale);
      expect(t.locale).toBe(locale);
      expect(Object.keys(t).sort()).toEqual(
        ["admin", "app", "catalog", "clinical", "config", "domain", "exports", "locale", "participant", "privacy", "research", "security"],
      );
    }
    expect(messagesFor("en").app.appName).toBe("Curv Console");
  });
});

describe("mock resolveLocale source: GET /users/me (I18N-16)", () => {
  it("a +ne user gets effective_locale ne, a plain user en (no org default, no header)", async () => {
    const api = createMockClient();
    const ne = await api.forToken(`mock-access.${btoa("clinician+ne@example.org")}`).getMyProfile();
    expect(ne.locale).toBe("ne");
    expect(ne.effective_locale).toBe("ne");
    const plain = await api.forToken(`mock-access.${btoa("clinician@example.org")}`).getMyProfile();
    expect(plain.locale).toBeNull();
    expect(plain.effective_locale).toBe("en");
  });

  it("PATCH stores the locale; null clears it back to the org default", async () => {
    const api = createMockClient().forToken(`mock-access.${btoa("admin@example.org")}`);
    expect((await api.updateMyProfile({ locale: "ne" })).effective_locale).toBe("ne");
    expect((await api.getMyProfile()).locale).toBe("ne");
    expect((await api.updateMyProfile({ locale: null })).effective_locale).toBe("en");
  });

  it("the org default applies to users without an override", async () => {
    const api = createMockClient().forToken(`mock-access.${btoa("admin@example.org")}`);
    await api.updateOrgClinicalSettings("5f0e1d2c-3b4a-4968-8776-655443322110", {}, { default_locale: "ne" });
    expect((await api.getMyProfile()).effective_locale).toBe("ne");
    expect((await api.getOrg("5f0e1d2c-3b4a-4968-8776-655443322110")).default_locale).toBe("ne");
  });
});

describe("problemText (I18N-19)", () => {
  const base = "https://schemas.curv.org/asd/problems/";
  it("uses the catalogue sentence for a known slug and keeps the server detail separate", () => {
    const p = { type: `${base}content-changed`, title: "Conflict", detail: "row_version mismatch" };
    const en = problemText(messagesFor("en"), p);
    expect(en.text).toContain("The content changed");
    expect(en.detail).toBe("row_version mismatch");
    const ne = problemText(messagesFor("ne"), p);
    expect(ne.text).toMatch(/[ऀ-ॿ]/);
    expect(ne.detail).toBe("row_version mismatch");
  });
  it("falls back to the server title for an unknown slug or a non-problem type", () => {
    expect(problemText(messagesFor("ne"), { type: `${base}no-such-slug`, title: "Server title", detail: null })).toEqual({
      text: "Server title",
      detail: null,
    });
    expect(problemText(messagesFor("en"), { type: "about:blank", title: "T", detail: "T" })).toEqual({ text: "T", detail: null });
  });
});

describe("format.ts (I18N-17)", () => {
  it("English output is unchanged", () => {
    expect(formatNumber(1234.5, 1)).toBe("1234.5");
    expect(formatNumber(null)).toBe("-");
    expect(formatPercent(0.456)).toBe("46%");
    const now = Date.parse("2026-01-02T00:00:00Z");
    expect(formatAge("2026-01-01T21:30:00Z", now)).toBe("2h 30m");
    expect(formatAge("2025-12-29T20:00:00Z", now)).toBe("3d 4h");
  });
  it("ne uses Latin digits, no grouping, and the bundle's unit words", () => {
    expect(formatNumber(1234.5, 1, "ne")).toBe("1234.5");
    expect(formatPercent(0.456, "ne")).toBe("46%");
    const units = messagesFor("ne").app.units;
    const now = Date.parse("2026-01-02T00:00:00Z");
    const age = formatAge("2026-01-01T21:30:00Z", now, "ne", units);
    expect(age).toBe(`2${units.hoursShort} 30${units.minutesShort}`);
    expect(`${formatNumber(12, 0, "ne")}${age}${formatPercent(0.12, "ne")}`).not.toMatch(/[०-९]/);
    expect(units.hoursShort).toMatch(/[ऀ-ॿ]/);
  });
});
