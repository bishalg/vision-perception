/** WP-C.23 (N-1, N-2, N-5): the visible navigation tree per role. */
import { describe, expect, it } from "vitest";
import { ROLES, type Role } from "@/lib/api";
import { messagesFor } from "@/lib/i18n/messages";
import { activeNav, navTree } from "@/lib/nav";

/** The tree as `division: [item paths below /o/]`. */
const shape = (role: Role) =>
  Object.fromEntries(navTree("/o", role).map((d) => [d.key, d.items.map((i) => i.href.slice("/o/".length))]));

const CLINIC_ALL = ["clinic/overview", "clinic/worklist", "clinic/participants", "clinic/follow-ups", "clinic/review", "clinic/sessions"];
const CLINIC_READ = ["clinic/overview", "clinic/participants", "clinic/review", "clinic/sessions"];
const RESEARCH = ["research/experiments", "research/cohorts", "research/datasets", "research/runs"];
const CONFIG = ["config/catalog", "config/baselines", "config/scoring", "config/sandbox", "config/bundles"];
const DATA_ALL = ["data/exports", "data/erasure", "data/retention", "data/audit"];
const ADMIN_ALL = ["admin/users", "admin/devices", "admin/integrations", "admin/clinical"];
const EVERYTHING = { clinic: CLINIC_ALL, research: RESEARCH, config: CONFIG, data: DATA_ALL, admin: ADMIN_ALL };

const EXPECTED: Record<Role, Record<string, string[]>> = {
  superadmin: EVERYTHING,
  owner: EVERYTHING,
  admin: EVERYTHING,
  clinician: {
    clinic: CLINIC_ALL,
    research: RESEARCH,
    config: CONFIG,
    data: ["data/exports"],
    admin: ["admin/devices", "admin/clinical"],
  },
  researcher: { clinic: CLINIC_READ, research: RESEARCH, config: CONFIG, data: ["data/exports"] },
  reviewer: { clinic: CLINIC_READ },
  viewer: { clinic: CLINIC_READ },
};

describe("navTree", () => {
  it("covers every role of ROLES", () => {
    expect(Object.keys(EXPECTED).sort()).toEqual([...ROLES].sort());
  });

  for (const role of ROLES) {
    it(`shows ${role} only the divisions and items it can use (N-1)`, () => {
      expect(shape(role)).toEqual(EXPECTED[role]);
    });
  }

  it("keeps the division order and never shows an empty division", () => {
    for (const role of ROLES) {
      const keys = navTree("/o", role).map((d) => d.key);
      expect(keys).toEqual(["clinic", "research", "config", "data", "admin"].filter((k) => keys.includes(k as never)));
      for (const d of navTree("/o", role)) expect(d.items.length).toBeGreaterThan(0);
    }
  });

  it("links each division to its landing page", () => {
    const hrefs = Object.fromEntries(navTree("/o", "owner").map((d) => [d.key, d.href]));
    expect(hrefs).toEqual({
      clinic: "/o/clinic/overview",
      research: "/o/research",
      config: "/o/config/catalog",
      data: "/o/data/exports",
      admin: "/o/admin/users",
    });
    // A division's landing page is its first visible item when that is not the role's first item.
    expect(navTree("/o", "clinician").find((d) => d.key === "admin")?.href).toBe("/o/admin/devices");
  });

  it("an unknown role sees the read-only clinic", () => {
    expect(navTree("/o", undefined).map((d) => d.key)).toEqual(["clinic"]);
  });

  it("takes its labels from the bundle (N-8)", () => {
    const en = navTree("/o", "owner").map((d) => d.text);
    expect(en).toEqual(["Clinic", "Research", "Configuration", "Data", "Administration"]);
    const ne = navTree("/o", "owner", messagesFor("ne"));
    expect(ne.map((d) => d.text)).not.toEqual(en);
    expect(ne[0]!.items[0]!.text).toBe(messagesFor("ne").app.nav.overview);
  });
});

describe("activeNav (N-2)", () => {
  const tree = navTree("/o", "owner");
  it("marks the division by prefix and the item by its own path, detail pages included", () => {
    const at = (p: string) => {
      const a = activeNav(tree, p);
      return [a.division?.key, a.item?.href];
    };
    expect(at("/o/clinic/sessions")).toEqual(["clinic", "/o/clinic/sessions"]);
    expect(at("/o/clinic/sessions/S1")).toEqual(["clinic", "/o/clinic/sessions"]);
    expect(at("/o/research/runs/r1/sessions")).toEqual(["research", "/o/research/runs"]);
    expect(at("/o/research")).toEqual(["research", undefined]);
    expect(at("/o/config/scoring/new")).toEqual(["config", "/o/config/scoring"]);
    expect(at("/o/admin/integrations/i1")).toEqual(["admin", "/o/admin/integrations"]);
    expect(at("/o/account/security")).toEqual([undefined, undefined]);
    // A look-alike segment is not the division.
    expect(at("/o/clinical")).toEqual([undefined, undefined]);
  });
});
