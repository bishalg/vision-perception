import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { ClipsTable } from "@/components/ClipsTable";
import { FlagBadge } from "@/components/FlagBadge";
import { KpiCard } from "@/components/KpiCard";
import { Pagination } from "@/components/Pagination";
import { SessionFilters } from "@/components/SessionFilters";
import { SessionsTable } from "@/components/SessionsTable";
import { ReportPanel } from "@/components/ReportPanel";
import { LoginForm } from "@/app/login/LoginForm";
import { MOCK_ORG_ID, createMockClient } from "@/lib/api/mock";
import type { ClipScore, MetricRow } from "@/lib/api";

describe("components", () => {
  it("KpiCard exposes label and value", () => {
    render(<KpiCard label="Sessions" value={12} />);
    expect(screen.getByLabelText("Sessions")).toHaveTextContent("12");
  });

  it("FlagBadge uses a text label, not only colour", () => {
    render(<FlagBadge flag="red" />);
    expect(screen.getByText("Red")).toHaveClass("badge-red");
  });

  it("SessionsTable renders rows linking to session detail, and an empty state", async () => {
    const { items } = await createMockClient().listSessions(MOCK_ORG_ID, { limit: 3 });
    const { rerender } = render(<SessionsTable orgId={MOCK_ORG_ID} sessions={items} />);
    expect(screen.getAllByRole("row")).toHaveLength(4);
    const first = screen.getAllByRole("link")[0]!;
    expect(first).toHaveAttribute("href", `/${MOCK_ORG_ID}/sessions/${items[0]!.id}`);
    rerender(<SessionsTable orgId={MOCK_ORG_ID} sessions={[]} />);
    expect(screen.getByText("No sessions match these filters.")).toBeInTheDocument();
  });

  it("SessionFilters preselects current values and submits via GET", () => {
    render(<SessionFilters orgId="o" query={{ flag: "red", from: "2026-10-01" }} />);
    expect(screen.getByLabelText("Flag")).toHaveValue("red");
    expect(screen.getByLabelText("From")).toHaveValue("2026-10-01");
    expect(screen.getByRole("form", { name: "Sessions" })).toHaveAttribute("method", "get");
  });

  it("Pagination links keep filters and add the cursor; first-page link drops it", () => {
    render(<Pagination orgId="o" query={{ flag: "red", cursor: "20" }} nextCursor="40" shown={20} total={64} />);
    expect(screen.getByRole("link", { name: "Next page" })).toHaveAttribute("href", "/o/sessions?flag=red&cursor=40");
    expect(screen.getByRole("link", { name: "First page" })).toHaveAttribute("href", "/o/sessions?flag=red");
    expect(screen.getByText("Showing 20 of 64 sessions")).toBeInTheDocument();
  });

  it("Pagination hides Next on the last page", () => {
    render(<Pagination orgId="o" query={{}} nextCursor={null} shown={4} total={4} />);
    expect(screen.queryByRole("link", { name: "Next page" })).toBeNull();
  });

  const row = (over: Partial<MetricRow> = {}): MetricRow => ({
    segment_id: "faces", metric: "eye_dwell_ratio", target_aoi: null, direction: "higherBetter",
    expected: 0.4, acceptable_low: 0.3, acceptable_high: null, recorded: 0.33, deviation: -0.07,
    status: "monitor", score: 0.7, imputed: false, source: "literature", ...over,
  });
  const clip = (over: Partial<ClipScore>): ClipScore => ({ clip_id: "c", order: 0, rows: [], ...over });

  it("ClipsTable renders one line per metric row, ordered by play order, with expected/recorded/deviation", () => {
    const clips = [
      clip({ clip_id: "second", order: 1, status: "monitor", rows: [row()] }),
      clip({
        clip_id: "first", order: 0, status: "atypical",
        rows: [row({ metric: "social_dwell_ratio", status: "onTarget" }), row({ metric: "eye_dwell_ratio", status: "atypical", recorded: 0.1, deviation: -0.3 })],
      }),
    ];
    render(<ClipsTable clips={clips} />);
    const rows = screen.getAllByRole("row").slice(1);
    expect(rows).toHaveLength(3);
    expect(within(rows[0]!).getByText("first")).toBeInTheDocument();
    expect(within(rows[0]!).getByText("social_dwell_ratio")).toBeInTheDocument();
    expect(within(rows[1]!).getByText("-0.30")).toBeInTheDocument();
    expect(within(rows[1]!).getByText("Atypical")).toHaveClass("badge-clip-atypical");
    expect(within(rows[2]!).getByText("second")).toBeInTheDocument();
    expect(within(rows[2]!).getByText("0.40")).toBeInTheDocument();
    expect(within(rows[2]!).getByText("0.33")).toBeInTheDocument();
  });

  it("ClipsTable marks imputed rows and shows a 'Not scored' line for a clip with null status and no rows", () => {
    render(
      <ClipsTable clips={[clip({ clip_id: "a", rows: [row({ imputed: true })], status: "monitor" }), clip({ clip_id: "b", order: 1, status: null })]} />,
    );
    expect(screen.getByText("(imputed)")).toBeInTheDocument();
    const rows = screen.getAllByRole("row").slice(1);
    expect(within(rows[1]!).getByText("Not scored")).toBeInTheDocument();
  });

  it("ClipsTable renders every mock clip, including null-status clips", async () => {
    const clips = await createMockClient().getSessionClips("SESS-MOCK-0001");
    render(<ClipsTable clips={[...clips].reverse()} />);
    const body = screen.getAllByRole("row").slice(1);
    expect(body).toHaveLength(clips.reduce((n, c) => n + Math.max(1, c.rows.length), 0));
    expect(within(body[0]!).getByText(clips[0]!.clip_id)).toBeInTheDocument();
  });

  it("ReportPanel offers a download only when a report exists", async () => {
    const s = await createMockClient().getSession("SESS-MOCK-0001");
    const { rerender } = render(<ReportPanel session={{ ...s, report_pdf_available: true }} />);
    expect(screen.getByRole("link", { name: "Download PDF report" })).toHaveAttribute("href", "/api/report/SESS-MOCK-0001");
    rerender(<ReportPanel session={{ ...s, report_pdf_available: false }} />);
    expect(screen.queryByRole("link")).toBeNull();
  });

  it("LoginForm shows an error returned by the action and submits credentials", async () => {
    const action = vi.fn().mockResolvedValue({ error: "failed" });
    render(<LoginForm action={action} next="/o/sessions" />);
    await userEvent.type(screen.getByLabelText("Email"), "a@b.org");
    await userEvent.type(screen.getByLabelText("Password"), "pw");
    await userEvent.click(screen.getByRole("button", { name: "Sign in" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Invalid email or password.");
    const fd = action.mock.calls[0]![1] as FormData;
    expect(fd.get("email")).toBe("a@b.org");
    expect(fd.get("next")).toBe("/o/sessions");
  });
});
