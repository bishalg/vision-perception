/** WP-L.1d: the users, catalog, config and research components render the Nepali tree under a `ne` locale provider. */
import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { ConfigStatusBadge, CatalogDiffView } from "@/components/catalog/CatalogTables";
import { BundleCompose } from "@/components/config/BundleCompose";
import { ParamControl } from "@/components/research/ParamControl";
import { SweepPanel } from "@/components/research/SweepPanel";
import { UsersTable } from "@/components/UsersAdmin";
import type { OrgUser } from "@/lib/api";
import type { ParamSpec } from "@/lib/api/research";
import { LocaleProvider } from "@/lib/i18n/provider";
import { assignableRoles } from "@/lib/permissions";
import { paramRegistry } from "@/lib/research/registry";
import { adminMessages } from "@/messages/admin";
import { catalogMessages } from "@/messages/catalog";
import { researchMessages } from "@/messages/research";

const user = (n: number, over: Partial<OrgUser> = {}): OrgUser => ({
  id: `u${n}`, email: `u${n}@x.org`, role: "viewer", org_id: "o", status: "active", last_login_at: null,
  display_name: null, professional_registration: null, ...over,
});
const spec = (id: string): ParamSpec => paramRegistry.params.find((p) => p.id === id)!;
const ne = (ui: React.ReactElement) => render(<LocaleProvider locale="ne">{ui}</LocaleProvider>);

describe("L.1d in Nepali", () => {
  it("UsersAdmin: role and status columns and the column headers come from the ne tree", () => {
    const t = adminMessages.ne.users;
    ne(<UsersTable selfId="u1" roles={assignableRoles("admin")} update={vi.fn()} canManage actor="admin" users={[user(2, { status: "disabled", role: "clinician" })]} />);
    expect(screen.getByText(t.statuses.disabled)).toBeInTheDocument();
    expect(screen.getByText(t.roles.clinician)).toBeInTheDocument();
    expect(screen.getByText(t.cols.status)).toBeInTheDocument();
    expect(screen.queryByText("Disabled")).not.toBeInTheDocument();
  });

  it("research: the registry label of metrics.ivtVelocityUnitsPerSec is the Nepali domain label", () => {
    ne(<ParamControl spec={spec("metrics.ivtVelocityUnitsPerSec")} value={0.5} published={0.5} context="edit" onChange={vi.fn()} />);
    expect(screen.getByText("फिक्सेसन गति सीमा")).toBeInTheDocument();
    expect(screen.queryByText("Fixation velocity threshold")).not.toBeInTheDocument();
  });

  it("research: SweepPanel takes the Nepali tree and the locale from its caller", () => {
    const sweep = {
      axes: [{ param_id: "metrics.ivtVelocityUnitsPerSec", values: [0.3, 0.5] }],
      published_values: [0.5],
      cells: [
        { values: [0.3], n: 20, composite: { mean: 0.6 }, flag_rate: { green: 0.5, yellow: 0.25, red: 0.25, unreliable: 0 }, metrics: {} },
        { values: [0.5], n: 20, composite: { mean: 0.7 }, flag_rate: { green: 0.6, yellow: 0.2, red: 0.2, unreliable: 0 }, metrics: {} },
      ],
    } as unknown as Parameters<typeof SweepPanel>[0]["sweep"];
    ne(<SweepPanel sweep={sweep} k={12} text={researchMessages.ne} locale="ne" />);
    expect(screen.getByRole("heading", { name: researchMessages.ne.analysis.sweepTitle })).toBeInTheDocument();
    expect(screen.getByText(/फिक्सेसन गति सीमा/)).toBeInTheDocument();
  });

  it("catalog: status badge, diff view and bundle compose read the ne trees", () => {
    const c = catalogMessages.ne;
    ne(
      <>
        <ConfigStatusBadge status="candidate" />
        <CatalogDiffView from="1" to="2" diff={{ fields: [], added: [], removed: [], changed: [] }} />
        <BundleCompose ready superadmin={false} compose={vi.fn()} />
      </>,
    );
    expect(screen.getByText(c.statuses.candidate)).toBeInTheDocument();
    expect(screen.getByText(c.diff.none)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /बन्डल बनाउनुहोस्/ })).toBeInTheDocument();
  });
});
