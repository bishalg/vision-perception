/** WP-C.23 (N-4, N-7): the URL tree and the redirects of the old flat paths. */
import { describe, expect, it } from "vitest";
import { LEGACY_REDIRECTS, NON_ORG_SEGMENTS, orgBase, orgPaths, pathsFor } from "@/lib/paths";
import config from "../next.config";

describe("paths", () => {
  const p = orgPaths("/o");

  it("puts every page under its division", () => {
    expect(p.root).toBe("/o");
    expect([p.clinic.overview, p.clinic.worklist, p.clinic.participants, p.clinic.followUps, p.clinic.review, p.clinic.sessions]).toEqual([
      "/o/clinic/overview",
      "/o/clinic/worklist",
      "/o/clinic/participants",
      "/o/clinic/follow-ups",
      "/o/clinic/review",
      "/o/clinic/sessions",
    ]);
    expect(p.clinic.participant("p1")).toBe("/o/clinic/participants/p1");
    expect(p.clinic.session("S1")).toBe("/o/clinic/sessions/S1");
    expect([p.research.root, p.research.experiments, p.research.newExperiment, p.research.cohorts, p.research.newCohort, p.research.runs]).toEqual([
      "/o/research",
      "/o/research/experiments",
      "/o/research/experiments/new",
      "/o/research/cohorts",
      "/o/research/cohorts/new",
      "/o/research/runs",
    ]);
    expect(p.research.experiment("e")).toBe("/o/research/experiments/e");
    expect(p.research.cohort("c")).toBe("/o/research/cohorts/c");
    expect(p.research.run("r")).toBe("/o/research/runs/r");
    expect(p.research.runSessions("r")).toBe("/o/research/runs/r/sessions");
    expect([p.config.catalog, p.config.baselines, p.config.scoring, p.config.sandbox, p.config.bundles]).toEqual([
      "/o/config/catalog",
      "/o/config/baselines",
      "/o/config/scoring",
      "/o/config/sandbox",
      "/o/config/bundles",
    ]);
    expect(p.config.doc("scoring")).toBe(p.config.scoring);
    expect(p.config.docNew("baselines")).toBe("/o/config/baselines/new");
    expect(p.config.docVersion("catalog", "2026.10.1")).toBe("/o/config/catalog/2026.10.1");
    expect([p.data.exports, p.data.erasure, p.data.retention, p.data.audit]).toEqual([
      "/o/data/exports",
      "/o/data/erasure",
      "/o/data/retention",
      "/o/data/audit",
    ]);
    expect(p.data.export("x")).toBe("/o/data/exports/x");
    expect([p.admin.users, p.admin.devices, p.admin.integrations, p.admin.clinical]).toEqual([
      "/o/admin/users",
      "/o/admin/devices",
      "/o/admin/integrations",
      "/o/admin/clinical",
    ]);
    expect(p.admin.integration("i")).toBe("/o/admin/integrations/i");
    expect(p.account.security).toBe("/o/account/security");
  });

  it("encodes the org and every id", () => {
    expect(orgBase("a b")).toBe("/a%20b");
    expect(pathsFor("a b").clinic.session("s/1")).toBe("/a%20b/clinic/sessions/s%2F1");
    expect(pathsFor("o").config.docVersion("scoring", "v 1")).toBe("/o/config/scoring/v%201");
  });
});

describe("legacy redirects (N-7)", () => {
  it("map every old flat path to its new home", () => {
    const map = Object.fromEntries(LEGACY_REDIRECTS.map((r) => [r.from, r.to]));
    expect(map).toEqual({
      overview: "clinic/overview",
      worklist: "clinic/worklist",
      participants: "clinic/participants",
      "follow-ups": "clinic/follow-ups",
      review: "clinic/review",
      sessions: "clinic/sessions",
      exports: "data/exports",
      erasure: "data/erasure",
      "settings/retention": "data/retention",
      audit: "data/audit",
      users: "admin/users",
      devices: "admin/devices",
      "settings/integrations": "admin/integrations",
      "settings/clinical": "admin/clinical",
      "settings/security": "account/security",
    });
  });

  it("every target is a page of the tree", () => {
    const p = orgPaths("/o");
    const pages = new Set<string>([
      ...Object.values(p.clinic),
      ...Object.values(p.data),
      ...Object.values(p.admin),
      ...Object.values(p.account),
    ].filter((v): v is string => typeof v === "string"));
    for (const r of LEGACY_REDIRECTS) expect(pages.has(`/o/${r.to}`)).toBe(true);
  });

  it("next.config turns them into permanent (308) redirects that skip the app's own top-level routes", async () => {
    const redirects = await config.redirects!();
    expect(redirects).toHaveLength(LEGACY_REDIRECTS.length);
    for (const r of redirects) expect(r).toMatchObject({ permanent: true });
    const sessions = redirects.find((r) => r.destination === "/:org/clinic/sessions/:path*")!;
    expect(sessions.source).toMatch(/^\/:org\(.*\)\/sessions\/:path\*$/);
    for (const seg of NON_ORG_SEGMENTS) expect(sessions.source).toContain(seg);
    expect(redirects.find((r) => r.destination === "/:org/account/security")?.source).toMatch(/\/settings\/security$/);
  });
});
