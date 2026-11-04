/** WP-B.23 (HIP-3): the audit page's "Export CSV" control and the action behind it. */
import { render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Role } from "@/lib/api";
import { MOCK_ORG_ID } from "@/lib/api/mock";

// One client for the page, the action and the spy (the mock hands out a fresh view per call).
const shared = vi.hoisted(() => ({ api: undefined as unknown as ReturnType<typeof import("@/lib/api").getApi> }));

vi.mock("server-only", () => ({}));
let role: Role | undefined = "owner";
vi.mock("@/lib/auth/server", async () => {
  const { getApi: api } = await import("@/lib/api");
  shared.api = api();
  return { authedApi: async () => shared.api, currentRole: async () => role };
});
vi.mock("@/lib/i18n/server", async () => {
  const { messagesFor } = await import("@/lib/i18n/messages");
  return { getText: async () => messagesFor("en") };
});
vi.mock("next/navigation", () => ({
  redirect: (to: string) => {
    throw Object.assign(new Error("NEXT_REDIRECT"), { to });
  },
  notFound: () => {
    throw new Error("NEXT_NOT_FOUND");
  },
  useRouter: () => ({ push: () => undefined, replace: () => undefined, refresh: () => undefined }),
}));

import AuditPage from "@/app/[org]/audit/page";
import { exportAuditCsvAction } from "@/app/[org]/audit/actions";
import { auditExportFilters } from "@/app/[org]/audit/filters";

const fd = (o: Record<string, string>) => {
  const f = new FormData();
  for (const [k, v] of Object.entries(o)) f.set(k, v);
  return f;
};

async function redirectOf(p: Promise<unknown>): Promise<string> {
  try {
    await p;
  } catch (e) {
    return (e as { to: string }).to;
  }
  throw new Error("expected a redirect");
}

async function renderPage(sp: Record<string, string> = {}) {
  render(await AuditPage({ params: Promise.resolve({ org: MOCK_ORG_ID }), searchParams: Promise.resolve(sp) }));
}

beforeEach(() => {
  role = "owner";
});

describe("audit page export control", () => {
  it("shows Export CSV to owner and admin only", async () => {
    for (const r of ["owner", "admin"] as const) {
      role = r;
      await renderPage();
      expect(screen.getByRole("button", { name: "Export CSV" })).toBeTruthy();
      document.body.innerHTML = "";
    }
    for (const r of ["clinician", "researcher", "viewer", "superadmin"] as const) {
      role = r;
      try {
        await renderPage();
      } catch {
        /* roles without audit.read see the denial */
      }
      expect(screen.queryByRole("button", { name: "Export CSV" })).toBeNull();
      document.body.innerHTML = "";
    }
  });

  it("carries the current filters and shows a failed export's reason", async () => {
    await renderPage({ actor_id: "u-1", action: "export.", export_error: "validation-failed" });
    const form = screen.getByRole("form", { name: "Export the audit log as CSV" });
    const val = (n: string) => (form.querySelector(`input[name="${n}"]`) as HTMLInputElement).value;
    expect(val("actor_id")).toBe("u-1");
    expect(val("action_prefix")).toBe("export.");
    expect(screen.getByRole("alert").textContent).toMatch(/not accepted/);
  });
});

describe("exportAuditCsvAction", () => {
  it("sends only the pinned request through createExport and lands on the export", async () => {
    const spy = vi.spyOn(shared.api, "createExport");
    const to = await redirectOf(
      exportAuditCsvAction(MOCK_ORG_ID, fd({ reason: "Quarterly audit review", actor_id: "", action_prefix: "export.", from: "2026-10-01", to: "" })),
    );
    expect(spy).toHaveBeenCalledWith(MOCK_ORG_ID, {
      format: "audit_csv",
      mode: "deidentified",
      purpose: "quality_improvement",
      reason: "Quarterly audit review",
      filters: { action_prefix: "export.", from: "2026-10-01" },
    });
    expect(to).toMatch(new RegExp(`^/${MOCK_ORG_ID}/exports/.+`));
  });

  it("sends nothing for a role without audit.export and returns to the page with the reason", async () => {
    role = "clinician";
    const spy = vi.spyOn(shared.api, "createExport");
    spy.mockClear();
    const to = await redirectOf(exportAuditCsvAction(MOCK_ORG_ID, fd({ reason: "Quarterly audit review" })));
    expect(spy).not.toHaveBeenCalled();
    expect(to).toContain("export_error=forbidden");
  });

  it("maps a backend refusal to an error key on the audit page", async () => {
    const to = await redirectOf(exportAuditCsvAction(MOCK_ORG_ID, fd({ reason: "short", actor_id: "u-1" })));
    expect(to).toContain(`/${MOCK_ORG_ID}/audit?`);
    expect(to).toContain("actor_id=u-1");
    expect(to).toMatch(/export_error=validation-failed/);
  });

  it("filters are limited to the four audit fields", () => {
    expect(auditExportFilters(fd({ from: "a", to: "b", actor_id: "c", action_prefix: "d", flag: "red" }))).toEqual({
      from: "a", to: "b", actor_id: "c", action_prefix: "d",
    });
  });
});
