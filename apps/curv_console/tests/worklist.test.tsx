import { render, screen, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { WorklistTable } from "@/components/clinical/WorklistTable";
import { ROLES } from "@/lib/api";
import type { WorklistItem } from "@/lib/api/clinical";
import { navLinks } from "@/lib/nav";
import { can } from "@/lib/permissions";
import { parseWorklistQuery } from "@/lib/worklistQuery";

const item = (n: number, over: Partial<WorklistItem> = {}): WorklistItem => ({
  id: `SESS-${n}`, participant_id: `p${n}`, participant_token: `child-00${n}`, started_at: "2026-11-01T08:00:00Z",
  duration_sec: 200, quality_verdict: "optimal", attention_flag: "red", age_hours: 5.5, assigned_to: null,
  clinical_state: null, cosign_required: true, has_patient_ref: true, review_id: null, ...over,
});

describe("WorklistTable", () => {
  it("renders state, cosign and patient-ref status and links to the session's clinical tab", () => {
    render(<WorklistTable orgId="org-1" items={[item(1), item(2, { clinical_state: "awaiting_cosign", assigned_to: "u", has_patient_ref: false, cosign_required: false })]} />);
    const rows = screen.getAllByRole("row").slice(1);
    expect(within(rows[0]!).getByText("Not started")).toBeInTheDocument();
    expect(within(rows[0]!).getByText("Required")).toBeInTheDocument();
    expect(within(rows[0]!).getByText("Linked")).toBeInTheDocument();
    expect(within(rows[0]!).getByText("Red")).toHaveClass("badge-red");
    expect(within(rows[0]!).getByRole("link", { name: "child-001" })).toHaveAttribute("href", "/org-1/sessions/SESS-1?tab=clinical");
    expect(within(rows[1]!).getByText("Awaiting cosign")).toBeInTheDocument();
  });

  it("shows an empty state", () => {
    render(<WorklistTable orgId="o" items={[]} />);
    expect(screen.getByText("Nothing is waiting for review.")).toBeInTheDocument();
  });

  it("puts no MRN, impression or free text in any href (UI-4): hrefs hold only the org and session ids", () => {
    const { container } = render(<WorklistTable orgId="org-1" items={[item(1), item(2), item(3)]} />);
    const hrefs = [...container.querySelectorAll("a")].map((a) => a.getAttribute("href")!);
    expect(hrefs.length).toBe(3);
    for (const h of hrefs) expect(h).toMatch(/^\/org-1\/sessions\/SESS-\d\?tab=clinical$/);
    // Nothing other than the pseudonymous token is rendered from the participant.
    expect(container.textContent).not.toMatch(/mrn\s*[:=]?\s*\d/i);
  });
});

describe("role-based nav and permissions", () => {
  const worklistLink = (role: Parameters<typeof navLinks>[1]) =>
    navLinks("/o", role).some((l) => l.href === "/o/worklist");

  it("shows Worklist to owner, admin and clinician, and hides it from viewer, researcher and reviewer", () => {
    for (const r of ["owner", "admin", "clinician"] as const) expect(worklistLink(r)).toBe(true);
    for (const r of ["viewer", "researcher", "reviewer"] as const) expect(worklistLink(r)).toBe(false);
    expect(worklistLink(undefined)).toBe(false);
  });

  it("mirrors the backend matrix for clinical content: owner and clinician read it, admin does not", () => {
    expect(can("owner", "clinical.content")).toBe(true);
    expect(can("clinician", "clinical.content")).toBe(true);
    expect(can("admin", "clinical.content")).toBe(false);
    expect(can("admin", "worklist.read")).toBe(true);
  });

  it("only clinicians write, sign and cosign; superadmin has none of the clinical content actions (RBAC-3)", () => {
    for (const a of ["clinical.write", "clinical.sign", "clinical.cosign", "clinical.content"] as const) {
      expect(can("superadmin", a)).toBe(false);
      expect(ROLES.filter((r) => can(r, a)).includes("viewer")).toBe(false);
    }
    expect(ROLES.filter((r) => can(r, "clinical.sign"))).toEqual(["clinician"]);
    expect(can("superadmin", "worklist.read")).toBe(true);
  });

  it("clinical settings are editable by owner and admin only", () => {
    expect(ROLES.filter((r) => can(r, "org.update")).sort()).toEqual(["admin", "owner", "superadmin"]);
  });
});

describe("parseWorklistQuery", () => {
  it("keeps enum values and the cursor and drops everything else", () => {
    expect(parseWorklistQuery({ state: "open", flag: "red", assigned: "me", cursor: "abc" })).toEqual({
      state: "open", flag: "red", assigned: "me", cursor: "abc",
    });
    expect(parseWorklistQuery({ state: "mrn-12345", flag: "<script>", assigned: ["bogus"], q: "impression text" })).toEqual({});
  });
});
