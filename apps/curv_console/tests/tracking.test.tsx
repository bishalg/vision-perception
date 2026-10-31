import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { DriftChart } from "@/components/DriftChart";
import { LossBar } from "@/components/LossBar";
import { QualityPanel } from "@/components/QualityPanel";
import { SessionFilters } from "@/components/SessionFilters";
import { SessionsTable } from "@/components/SessionsTable";
import { MOCK_ORG_ID, createMockClient } from "@/lib/api/mock";
import { parseSessionQuery } from "@/lib/sessionQuery";
import type { SessionDetail } from "@/lib/api";

async function sessions() {
  const api = createMockClient();
  const { items } = await api.listSessions(MOCK_ORG_ID, { limit: 100 });
  return Promise.all(items.map((s) => api.getSession(s.id)));
}

describe("tracking quality UI (WP-G.12)", () => {
  it("a 1.6 session shows Not measured, not zeros, and a null distance shows Not measured", async () => {
    const legacy = (await sessions()).find((s) => !s.tracking_quality)!;
    render(<QualityPanel session={{ ...legacy, calibrated_distance_cm: null }} />);
    expect(screen.getByText(/Not measured \(app version before tracking quality\)/)).toBeInTheDocument();
    expect(screen.getByText("Calibrated distance (cm)").nextSibling).toHaveTextContent("Not measured");
    expect(screen.queryByText(/0\.00/)).toBeNull();
  });

  it("a 1.7 session renders verdict, accuracy, calibration, loss bar, drift chart and conditions", async () => {
    const s = (await sessions()).find((x) => x.tracking_verdict === "poor")!;
    render(<QualityPanel session={s} />);
    expect(screen.getByText("Tracking verdict").nextSibling).toHaveTextContent("Poor");
    expect(screen.getByText("Estimated accuracy").nextSibling).toHaveTextContent("3.90°");
    expect(screen.getByText("Calibration model").nextSibling).toHaveTextContent("screenAffine");
    expect(screen.getByRole("img", { name: /Tracked: .*Blink: .*Lost: .*Off-screen: .*Artefact:/ })).toBeInTheDocument();
    expect(screen.getByRole("img", { name: /Gaze offset in degrees at 3 inter-clip checks/ })).toBeInTheDocument();
    expect(screen.getByText("Glasses")).toBeInTheDocument();
  });

  it("an unusable session with no median distance says Not measured, never 35 cm", async () => {
    const s = (await sessions()).find((x) => x.tracking_verdict === "unusable")!;
    render(<QualityPanel session={s as SessionDetail} />);
    expect(screen.getByText("Distance, median (cm)").nextSibling).toHaveTextContent("Not measured");
    expect(screen.queryByText(/35/)).toBeNull();
  });

  it("DriftChart draws warn and hard lines only from the thresholds it is given", async () => {
    const checks = [
      { at_sec: 10, x_norm: 0.5, y_norm: 0.5, missed: false, offset_deg: 1, drift_deg: 1 },
      { at_sec: 20, x_norm: 0.5, y_norm: 0.5, missed: true, offset_deg: null, drift_deg: null },
    ];
    const { rerender } = render(<DriftChart checks={checks} />);
    expect(screen.queryByTestId("drift-warn")).toBeNull();
    rerender(<DriftChart checks={checks} warnDeg={1.5} hardDeg={3} />);
    expect(screen.getByTestId("drift-warn")).toHaveTextContent("Warn 1.5°");
    expect(screen.getByTestId("drift-hard")).toHaveTextContent("Hard limit 3°");
    expect(screen.getByText("20.0 s: missed")).toBeInTheDocument();
  });

  it("DriftChart with no checks says so", () => {
    render(<DriftChart checks={[]} />);
    expect(screen.getByText("No drift checks recorded.")).toBeInTheDocument();
  });

  it("LossBar lists every track state", () => {
    render(
      <LossBar
        durationSec={100}
        loss={{ track_loss_sec: 5, blink_sec: 2, offscreen_sec: 3, artifact_sec: 0, gap_filled_sec: 1, longest_gap_sec: 1 }}
      />,
    );
    expect(screen.getByText("Tracked: 90.0 s")).toBeInTheDocument();
    expect(screen.getByText("Lost: 5.0 s")).toBeInTheDocument();
  });

  it("sessions table has a tracking column; 1.6 rows say Not measured", async () => {
    const api = createMockClient();
    const { items } = await api.listSessions(MOCK_ORG_ID, { limit: 6 });
    render(<SessionsTable orgId={MOCK_ORG_ID} sessions={items} />);
    expect(screen.getByRole("columnheader", { name: "Tracking" })).toBeInTheDocument();
    expect(screen.getAllByText("Good").length + screen.getAllByText("Not measured").length).toBeGreaterThan(0);
  });

  it("filter select and URL parsing accept only the four verdicts", () => {
    render(<SessionFilters orgId={MOCK_ORG_ID} query={{ tracking: "fair" }} />);
    expect(screen.getByLabelText("Tracking")).toHaveValue("fair");
    expect(parseSessionQuery({ tracking: "poor" }).tracking).toBe("poor");
    expect(parseSessionQuery({ tracking: "bogus" }).tracking).toBeUndefined();
  });

  it("mock list filters by tracking verdict and the review queue carries low_tracking_quality", async () => {
    const api = createMockClient();
    const poor = await api.listSessions(MOCK_ORG_ID, { tracking: "poor", limit: 100 });
    expect(poor.items.length).toBeGreaterThan(0);
    expect(poor.items.every((s) => s.tracking_verdict === "poor")).toBe(true);
    const review = await api.listReview(MOCK_ORG_ID, { limit: 100 });
    expect(review.items.some((r) => r.reason === "low_tracking_quality")).toBe(true);
  });
});
