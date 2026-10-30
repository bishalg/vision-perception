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

  it("ClipsTable orders rows by play order and shows status text", async () => {
    const clips = await createMockClient().getSessionClips("sess-0001");
    render(<ClipsTable clips={[...clips].reverse()} />);
    const rows = screen.getAllByRole("row").slice(1);
    expect(within(rows[0]!).getByText(clips[0]!.clip_id)).toBeInTheDocument();
    expect(rows).toHaveLength(clips.length);
  });

  it("ReportPanel offers a download only when a report exists", async () => {
    const s = await createMockClient().getSession("sess-0001");
    const { rerender } = render(<ReportPanel session={{ ...s, report_pdf_available: true }} />);
    expect(screen.getByRole("link", { name: "Download PDF report" })).toHaveAttribute("href", "/api/report/sess-0001");
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
