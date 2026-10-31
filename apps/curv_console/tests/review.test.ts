import { describe, expect, it } from "vitest";
import { CAPTURE_ISSUE_LABELS, TRIAGE_ACTIONS, triageForKey, triagePatch } from "@/lib/review";
import { can } from "@/lib/permissions";
import { MAX_RECENT_ORGS, orgFromPath, parseRecentOrgs, rememberOrg } from "@/lib/orgs";
import { formatAge } from "@/lib/format";
import { updateReviewRequestSchema } from "@/lib/api";

describe("triage actions (spec 03 §4.2)", () => {
  it("are the four spec actions bound to keys 1-4, in order", () => {
    expect(TRIAGE_ACTIONS).toEqual(["confirm", "capture_issue", "exclude_from_norms", "dismiss"]);
    expect(["1", "2", "3", "4"].map(triageForKey)).toEqual([...TRIAGE_ACTIONS]);
    expect(triageForKey("5")).toBeNull();
    expect(triageForKey("j")).toBeNull();
  });

  it("Confirm closes the item as done and writes a triage label", () => {
    expect(triagePatch("confirm")).toEqual({
      status: "done",
      decision: { action: "confirm" },
      labels: [{ key: "triage", value: "confirm" }],
    });
  });

  it("Flag capture issue needs a known label and records it", () => {
    expect(() => triagePatch("capture_issue")).toThrow(/label/);
    expect(() => triagePatch("capture_issue", "bogus" as never)).toThrow(/label/);
    const patch = triagePatch("capture_issue", "adult_in_frame");
    expect(patch.status).toBe("done");
    expect(patch.decision).toEqual({ action: "capture_issue", capture_issue: "adult_in_frame" });
    expect(patch.labels).toContainEqual({ key: "capture_issue", value: "adult_in_frame" });
    expect(CAPTURE_ISSUE_LABELS).toContain("calibration_drift");
  });

  it("Exclude from norms uses the keys the norm-aggregation job reads (label and decision)", () => {
    const patch = triagePatch("exclude_from_norms");
    expect(patch.status).toBe("done");
    expect(patch.decision).toMatchObject({ exclude_from_norms: true });
    expect(patch.labels).toContainEqual({ key: "exclude_from_norms", value: true });
  });

  it("Dismiss sets status dismissed", () => {
    expect(triagePatch("dismiss")).toMatchObject({ status: "dismissed", decision: { action: "dismiss" } });
  });

  it("every patch is a valid UpdateReviewRequest", () => {
    for (const a of TRIAGE_ACTIONS) {
      expect(updateReviewRequestSchema.safeParse(triagePatch(a, "other")).success).toBe(true);
    }
  });
});

describe("UI permissions mirror the backend RBAC matrix (hiding only; the backend enforces)", () => {
  it("review write: owner, admin, clinician, reviewer, superadmin", () => {
    expect(can("reviewer", "review.write")).toBe(true);
    expect(can("superadmin", "review.write")).toBe(true);
    expect(can("viewer", "review.write")).toBe(false);
    expect(can("researcher", "review.write")).toBe(false);
  });
  it("devices: read and enrol for clinicians, revoke for owner/admin only", () => {
    expect(can("clinician", "device.read")).toBe(true);
    expect(can("clinician", "enrol.create")).toBe(true);
    expect(can("clinician", "device.revoke")).toBe(false);
    expect(can("admin", "device.revoke")).toBe(true);
    expect(can("reviewer", "device.read")).toBe(false);
  });
  it("an unknown role (no cookie) is allowed nothing", () => {
    expect(can(undefined, "review.write")).toBe(false);
  });
});

describe("recent orgs (org switcher)", () => {
  const a = "5f0e1d2c-3b4a-4968-8776-655443322110";
  const b = "11111111-2222-4333-8444-555555555555";
  it("takes the org from an org-scoped path only when it is a UUID", () => {
    expect(orgFromPath(`/${a}/sessions/x`)).toBe(a);
    expect(orgFromPath("/login")).toBeNull();
    expect(orgFromPath("/api/report/x")).toBeNull();
    expect(orgFromPath("/")).toBeNull();
  });
  it("puts the latest org first, dedupes, caps the list and drops junk", () => {
    expect(rememberOrg(undefined, a)).toBe(a);
    expect(parseRecentOrgs(rememberOrg(`${a},${b}`, b))).toEqual([b, a]);
    expect(parseRecentOrgs(`${a},<script>,${b}`)).toEqual([a, b]);
    let v: string | undefined;
    for (let i = 0; i < 10; i++) v = rememberOrg(v, `00000000-0000-4000-8000-00000000000${i}`);
    expect(parseRecentOrgs(v)).toHaveLength(MAX_RECENT_ORGS);
  });
});

describe("formatAge (SLA age in the queue)", () => {
  const now = Date.parse("2026-11-01T12:00:00Z");
  it("formats minutes, hours and days", () => {
    expect(formatAge("2026-11-01T11:55:00Z", now)).toBe("5m");
    expect(formatAge("2026-11-01T09:30:00Z", now)).toBe("2h 30m");
    expect(formatAge("2026-10-29T08:00:00Z", now)).toBe("3d 4h");
    expect(formatAge("garbage", now)).toBe("-");
  });
});

describe("review list URL state", async () => {
  const { parseReviewQuery, toSearch } = await import("@/lib/listQuery");
  it("defaults to the open queue, accepts status=all, and drops unknown values", () => {
    expect(parseReviewQuery({})).toEqual({ status: "open" });
    expect(parseReviewQuery({ status: "all" })).toEqual({ all: true });
    expect(parseReviewQuery({ status: "done", reason: "flag_red", cursor: "c" })).toEqual({ status: "done", reason: "flag_red", cursor: "c" });
    expect(parseReviewQuery({ status: "closed", reason: "quality_low" })).toEqual({ status: "open" });
  });
  it("serialises only set params", () => {
    expect(toSearch({ status: "open", reason: undefined })).toBe("?status=open");
    expect(toSearch({})).toBe("");
  });
});
