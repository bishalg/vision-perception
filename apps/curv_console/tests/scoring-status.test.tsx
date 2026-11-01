/**
 * SCH-5 (session schema 1.8): an `unscorable` session shows "Not scored (reason)" instead of score numbers in the
 * session list, the quality panel and the report panel; the zod schemas decode 1.8 summaries and details.
 */
import { render, screen, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { QualityPanel } from "@/components/QualityPanel";
import { ReportPanel } from "@/components/ReportPanel";
import { notScoredLabel } from "@/components/ScoringStatus";
import { SessionsTable } from "@/components/SessionsTable";
import { MOCK_ORG_ID, createMockClient } from "@/lib/api/mock";
import { REVIEW_REASONS, sessionDetailSchema, sessionSummarySchema } from "@/lib/api";

async function details() {
  const api = createMockClient();
  const { items } = await api.listSessions(MOCK_ORG_ID, { limit: 100 });
  return { items, details: await Promise.all(items.map((s) => api.getSession(s.id))) };
}

describe("scoring status (SCH-5)", () => {
  it("labels unscorable sessions with their reason and leaves scored ones alone", () => {
    expect(notScoredLabel({ scoring_status: "unscorable", unscorable_reason: "aggregation_failed" })).toBe(
      "Not scored (aggregation failed)",
    );
    expect(notScoredLabel({ scoring_status: "unscorable", unscorable_reason: null })).toBe("Not scored");
    expect(notScoredLabel({ scoring_status: "scored", unscorable_reason: null })).toBeNull();
    expect(notScoredLabel({ scoring_status: null })).toBeNull();
    expect(notScoredLabel({})).toBeNull();
    expect(REVIEW_REASONS).toContain("unscorable");
  });

  it("decodes 1.8 summaries and details; 1.6/1.7 payloads without the fields still decode", () => {
    const summary = sessionSummarySchema.parse({
      id: "S",
      participant_id: "p",
      participant_token: "t",
      started_at: "2026-11-01T10:00:00.000Z",
      duration_sec: 30,
      quality_verdict: "optimal",
      attention_flag: "unreliable",
      scoring_status: "unscorable",
      unscorable_reason: "tracking_unusable",
    });
    expect(summary.unscorable_reason).toBe("tracking_unusable");
    expect(() => sessionSummarySchema.parse({ ...summary, scoring_status: "failed" })).toThrow();
    const legacy = sessionDetailSchema.parse({
      ...summary,
      scoring_status: undefined,
      unscorable_reason: undefined,
      org_id: "o",
    });
    expect(legacy.scoring_status).toBeUndefined();
  });

  it("the session list shows Not scored instead of the composite", async () => {
    const { items } = await details();
    const unscorable = items.filter((s) => s.scoring_status === "unscorable");
    expect(unscorable.length).toBeGreaterThan(0);
    render(<SessionsTable orgId={MOCK_ORG_ID} sessions={unscorable} />);
    const rows = screen.getAllByRole("row").slice(1);
    expect(rows).toHaveLength(unscorable.length);
    for (const row of rows) {
      expect(within(row).getByText(/^Not scored/)).toBeInTheDocument();
    }
    expect(screen.getByText("Not scored (aggregation failed)")).toBeInTheDocument();
  });

  it("the quality and report panels say Not scored and print no social preference number", async () => {
    const s = (await details()).details.find((d) => d.unscorable_reason === "aggregation_failed")!;
    expect(s.social_preference).toBeNull();
    const { unmount } = render(<QualityPanel session={s} />);
    expect(screen.getByText("Social preference").nextSibling).toHaveTextContent("Not scored (aggregation failed)");
    unmount();
    render(<ReportPanel session={{ ...s, capture_source: "truedepth" }} />);
    expect(screen.getByRole("status")).toHaveTextContent("Not scored (aggregation failed).");
    expect(screen.getByRole("link", { name: "Download PDF report" })).toBeVisible();
  });

  it("a scored session keeps its numbers and no Not scored note", async () => {
    const s = (await details()).details.find((d) => d.scoring_status !== "unscorable" && d.social_preference != null)!;
    render(<QualityPanel session={s} />);
    expect(screen.getByText("Social preference").nextSibling).toHaveTextContent(
      `${Math.round(s.social_preference! * 100)}%`,
    );
    expect(screen.queryByText(/Not scored/)).toBeNull();
  });
});
