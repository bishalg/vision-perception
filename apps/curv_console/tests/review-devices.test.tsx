import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { ReviewQueue, type ReviewRow } from "@/components/ReviewQueue";
import { QrCode } from "@/components/QrCode";
import { EnrolCodePanel } from "@/components/EnrolCodePanel";
import { DevicesTable } from "@/components/DevicesTable";
import { OrgSwitcher } from "@/components/OrgSwitcher";
import type { Device, ReviewItem } from "@/lib/api";

const push = vi.fn();
vi.mock("next/navigation", () => ({ useRouter: () => ({ push, refresh: vi.fn() }) }));

const NOW = Date.parse("2026-11-01T12:00:00Z");

function row(n: number, over: Partial<ReviewItem> = {}): ReviewRow {
  return {
    item: {
      id: `r${n}`, org_id: "o", session_id: `S${n}`, reason: n % 2 ? "flag_red" : "low_quality", status: "open",
      created_at: new Date(NOW - n * 3_600_000).toISOString(), assigned_to: null, decision: null,
      decided_by: null, decided_at: null, ...over,
    },
    session: {
      participant_token: `child-00${n}`, attention_flag: n % 2 ? "red" : "green", quality_verdict: "degraded",
      started_at: new Date(NOW - n * 3_700_000).toISOString(),
    },
  };
}

const selectedToken = () => {
  const r = screen.getAllByRole("row").find((tr) => tr.getAttribute("aria-current") === "true");
  return r ? within(r).getAllByRole("cell")[0]!.textContent : null;
};

describe("ReviewQueue", () => {
  beforeEach(() => push.mockReset());

  it("renders reasons, flags and SLA age, selects the first row, and links each session", () => {
    render(<ReviewQueue orgId="o" rows={[row(1), row(2)]} canWrite now={NOW} filterStatus="open" triage={vi.fn()} />);
    const rows = screen.getAllByRole("row").slice(1);
    expect(rows).toHaveLength(2);
    expect(within(rows[0]!).getByText("Red flag")).toBeInTheDocument();
    expect(within(rows[0]!).getByText("Red")).toHaveClass("badge-red");
    expect(within(rows[0]!).getByText("1h 0m")).toBeInTheDocument();
    expect(within(rows[0]!).getByRole("link", { name: "child-001" })).toHaveAttribute("href", "/o/sessions/S1");
    expect(selectedToken()).toBe("child-001");
  });

  it("j / k move the selection and o opens the selected session", async () => {
    const user = userEvent.setup();
    render(<ReviewQueue orgId="o" rows={[row(1), row(2), row(3)]} canWrite now={NOW} triage={vi.fn()} />);
    await user.keyboard("j");
    expect(selectedToken()).toBe("child-002");
    await user.keyboard("jjj");
    expect(selectedToken()).toBe("child-003");
    await user.keyboard("k");
    expect(selectedToken()).toBe("child-002");
    await user.keyboard("o");
    expect(push).toHaveBeenCalledWith("/o/sessions/S2");
  });

  it("key 1 confirms the selected item, removes it from an open-only view and announces it", async () => {
    const user = userEvent.setup();
    const triage = vi.fn().mockResolvedValue({ ok: true, item: { ...row(1).item, status: "done" } });
    render(<ReviewQueue orgId="o" rows={[row(1), row(2)]} canWrite now={NOW} filterStatus="open" triage={triage} />);
    await user.keyboard("1");
    expect(triage).toHaveBeenCalledWith("r1", "confirm", undefined);
    await waitFor(() => expect(screen.getAllByRole("row")).toHaveLength(2));
    expect(selectedToken()).toBe("child-002");
    expect(screen.getByRole("status")).toHaveTextContent("Confirm: child-001");
  });

  it("toolbar buttons do the same as keys 3 and 4", async () => {
    const user = userEvent.setup();
    const triage = vi.fn().mockImplementation(async (id: string) => ({ ok: true, item: { ...row(1).item, id, status: "done" } }));
    render(<ReviewQueue orgId="o" rows={[row(1), row(2)]} canWrite now={NOW} triage={triage} />);
    await user.click(screen.getByRole("button", { name: /Exclude from norms/ }));
    expect(triage).toHaveBeenLastCalledWith("r1", "exclude_from_norms", undefined);
    await user.keyboard("j4");
    expect(triage).toHaveBeenLastCalledWith("r2", "dismiss", undefined);
  });

  it("key 2 asks for a capture-issue label; keys typed in the picker do not trigger triage", async () => {
    const user = userEvent.setup();
    const triage = vi.fn().mockResolvedValue({ ok: true, item: { ...row(1).item, status: "done" } });
    render(<ReviewQueue orgId="o" rows={[row(1)]} canWrite now={NOW} filterStatus="open" triage={triage} />);
    await user.keyboard("2");
    const picker = screen.getByLabelText("Capture issue");
    expect(picker).toHaveFocus();
    await user.selectOptions(picker, "adult_in_frame");
    await user.type(picker, "1");
    expect(triage).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "Apply label" }));
    expect(triage).toHaveBeenCalledWith("r1", "capture_issue", "adult_in_frame");
  });

  it("Escape closes the capture picker without acting", async () => {
    const user = userEvent.setup();
    const triage = vi.fn();
    render(<ReviewQueue orgId="o" rows={[row(1)]} canWrite now={NOW} triage={triage} />);
    await user.keyboard("2");
    await user.keyboard("{Escape}");
    expect(screen.queryByLabelText("Capture issue")).toBeNull();
    expect(triage).not.toHaveBeenCalled();
  });

  it("a failed action keeps the row and shows an alert", async () => {
    const user = userEvent.setup();
    const triage = vi.fn().mockResolvedValue({ ok: false, error: "forbidden" });
    render(<ReviewQueue orgId="o" rows={[row(1)]} canWrite now={NOW} filterStatus="open" triage={triage} />);
    await user.keyboard("1");
    expect(await screen.findByRole("alert")).toHaveTextContent("The action failed");
    expect(screen.getAllByRole("row")).toHaveLength(2);
  });

  it("read-only roles get no toolbar and keys 1-4 do nothing", async () => {
    const user = userEvent.setup();
    const triage = vi.fn();
    render(<ReviewQueue orgId="o" rows={[row(1)]} canWrite={false} now={NOW} triage={triage} />);
    expect(screen.queryByRole("toolbar")).toBeNull();
    expect(screen.getByText(/can view the queue but not triage/)).toBeInTheDocument();
    await user.keyboard("1");
    expect(triage).not.toHaveBeenCalled();
  });

  it("shows an empty state, and a placeholder when session details could not be loaded", () => {
    const { unmount } = render(<ReviewQueue orgId="o" rows={[]} canWrite now={NOW} triage={vi.fn()} />);
    expect(screen.getByText("The queue is empty for these filters.")).toBeInTheDocument();
    unmount();
    render(<ReviewQueue orgId="o" rows={[{ item: row(1).item, session: null }]} canWrite now={NOW} triage={vi.fn()} />);
    expect(screen.getByRole("link", { name: "S1" })).toHaveAttribute("href", "/o/sessions/S1");
  });
});

describe("QrCode", () => {
  it("renders a labelled, deterministic SVG for the value", () => {
    const { container, rerender } = render(<QrCode value="K7Q4-9XMP" label="QR for K7Q4-9XMP" />);
    const svg = screen.getByRole("img", { name: "QR for K7Q4-9XMP" });
    const d = svg.querySelector("path")!.getAttribute("d")!;
    expect(d.length).toBeGreaterThan(100);
    rerender(<QrCode value="K7Q4-9XMP" label="QR for K7Q4-9XMP" />);
    expect(container.querySelector("path")!.getAttribute("d")).toBe(d);
    rerender(<QrCode value="ABCD-EFGH" label="x" />);
    expect(container.querySelector("path")!.getAttribute("d")).not.toBe(d);
  });
});

describe("EnrolCodePanel", () => {
  it("submits the form and shows the code once with its QR", async () => {
    const user = userEvent.setup();
    const create = vi.fn().mockResolvedValue({
      ok: true, code: "K7Q4-9XMP", max_uses: 2, expires_at: "2026-11-06T12:00:00.000Z",
    });
    render(<EnrolCodePanel create={create} canCreate />);
    await user.type(screen.getByLabelText(/Label/), "Room 2");
    await user.clear(screen.getByLabelText("Max uses"));
    await user.type(screen.getByLabelText("Max uses"), "2");
    await user.click(screen.getByRole("button", { name: "Generate enrol code" }));
    expect(await screen.findByText("K7Q4-9XMP")).toBeInTheDocument();
    expect(screen.getByRole("img", { name: "QR code for enrol code K7Q4-9XMP" })).toBeInTheDocument();
    expect(screen.getByText(/shown only once/)).toBeInTheDocument();
    const fd = create.mock.calls[0]![0] as FormData;
    expect(fd.get("label")).toBe("Room 2");
    expect(fd.get("max_uses")).toBe("2");
  });

  it("shows an alert on failure and hides the form for roles that cannot enrol", async () => {
    const user = userEvent.setup();
    const create = vi.fn().mockResolvedValue({ ok: false, error: "max_uses: must be 1 to 1000" });
    const { rerender } = render(<EnrolCodePanel create={create} canCreate />);
    await user.click(screen.getByRole("button", { name: "Generate enrol code" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("max_uses: must be 1 to 1000");
    rerender(<EnrolCodePanel create={create} canCreate={false} />);
    expect(screen.queryByRole("button", { name: "Generate enrol code" })).toBeNull();
  });
});

describe("DevicesTable", () => {
  const device = (over: Partial<Device>): Device => ({
    id: "d1", org_id: "o", platform: "ios", app_version: "1.4.0", enrolled_at: "2026-10-01T00:00:00.000Z",
    label: "iPad room 1", revoked_at: null, last_seen_at: null, config_bundle_version: "2026.11.0", ...over,
  });

  it("shows status as text and a two-step revoke only for live devices", async () => {
    const user = userEvent.setup();
    const revoke = vi.fn().mockResolvedValue({ ok: true });
    render(
      <DevicesTable
        devices={[device({}), device({ id: "d2", label: "Lost", revoked_at: "2026-10-20T00:00:00.000Z" })]}
        canRevoke
        revoke={revoke}
      />,
    );
    const [live, gone] = screen.getAllByRole("row").slice(1);
    expect(within(gone!).getByText("Revoked")).toBeInTheDocument();
    expect(within(gone!).queryByRole("button")).toBeNull();
    expect(within(live!).getByText("Active")).toBeInTheDocument();
    await user.click(within(live!).getByRole("button", { name: "Revoke" }));
    expect(revoke).not.toHaveBeenCalled();
    await user.click(within(live!).getByRole("button", { name: "Confirm revoke" }));
    await act(async () => {});
    expect(revoke).toHaveBeenCalledWith("d1");
  });

  it("hides revoke for roles that cannot revoke, and has an empty state", () => {
    const { rerender } = render(<DevicesTable devices={[device({})]} canRevoke={false} revoke={vi.fn()} />);
    expect(screen.queryByRole("button", { name: "Revoke" })).toBeNull();
    rerender(<DevicesTable devices={[]} canRevoke revoke={vi.fn()} />);
    expect(screen.getByText("No devices are enrolled yet.")).toBeInTheDocument();
  });
});

describe("OrgSwitcher", () => {
  const orgs = [{ id: "a", name: "Clinic A" }, { id: "b", name: "Clinic B" }];
  it("submits a GET form to /switch-org with the current org preselected", () => {
    render(<OrgSwitcher current="b" orgs={orgs} allowAnyId={false} />);
    expect(screen.getByLabelText("Organisation")).toHaveValue("b");
    expect(screen.getByRole("form", { name: "Switch organisation" })).toHaveAttribute("action", "/switch-org");
    expect(screen.queryByLabelText("Organisation id")).toBeNull();
  });
  it("offers an org-id field to the superadmin", () => {
    render(<OrgSwitcher current="a" orgs={orgs} allowAnyId />);
    expect(screen.getByLabelText("Organisation id")).toBeInTheDocument();
  });
});
