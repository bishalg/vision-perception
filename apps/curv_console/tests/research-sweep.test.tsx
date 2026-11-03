/**
 * WP-X.9: the sweep and age-profile sections of a run page (SWP-3, AGE-1, AGE-3, PRV-1). One axis draws curves with
 * the published value marked and a table of every cell; two axes draw heatmaps of every grid cell (the off-grid
 * published cell listed, not drawn); the age tables show the shares under both baselines and hide small bands as
 * "n < t". The mock seeds parse with the run schema.
 */
import { render, screen, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { AgeProfileTable } from "@/components/research/AgeProfileTable";
import { SweepPanel } from "@/components/research/SweepPanel";
import { analysisRunSchema, type RunSummary } from "@/lib/api/research";
import { createMockClient } from "@/lib/api/mock";
import { MOCK_RESEARCH_IDS } from "@/lib/api/mock/research";
import { parseAgeProfile } from "@/lib/research/age-profile";
import { gridLayout, isPublishedCell, orderedCells, shadeStep, type SweepSummary } from "@/lib/research/run-sweep";
import { researchMessages } from "@/messages/research";

const en = researchMessages.en;
const ne = researchMessages.ne;

type Cell = SweepSummary["cells"][number];
const cell = (values: number[], n: number | null, composite: number, green: number): Cell => ({
  values,
  n,
  flag_rate: n == null ? null : { green, yellow: 1 - green, red: 0, unreliable: 0 },
  quality_rate: { good: n == null ? null : 1 },
  metrics: n == null ? {} : { eye_dwell_ratio: { mean: composite / 2, median: composite / 2 } },
  composite: n == null ? { mean: null, median: null } : { mean: composite, median: composite },
  changed_vs_published: n == null ? null : 0,
});

const oneAxis: SweepSummary = {
  axes: [{ param_id: "flag.greenFloor", from: 0.5, to: 0.8, step: 0.1 }],
  published_values: [0.65],
  // Grid order from the core, the off-grid published cell last.
  cells: [cell([0.5], 30, 0.6, 0.8), cell([0.6], 30, 0.6, 0.7), cell([0.7], 5, 0.6, 0.5), cell([0.8], null, 0, 0), cell([0.65], 30, 0.6, 0.6)],
};

async function seeded(id: string) {
  const run = await createMockClient().getAnalysisRun(id);
  return analysisRunSchema.parse(run);
}

describe("sweep helpers", () => {
  it("orders cells, finds the published one and lays the grid out", () => {
    expect(orderedCells(oneAxis).map((c) => c.values[0])).toEqual([0.5, 0.6, 0.65, 0.7, 0.8]);
    expect(oneAxis.cells.filter((c) => isPublishedCell(oneAxis, c))).toHaveLength(1);
    const two: SweepSummary = {
      axes: [oneAxis.axes[0]!, { param_id: "flag.yellowFloor", from: 0.1, to: 0.2, step: 0.1 }],
      published_values: [0.5, 0.2],
      cells: [cell([0.5, 0.1], 30, 0.5, 0.5), cell([0.5, 0.2], 30, 0.6, 0.5), cell([0.6, 0.1], 30, 0.7, 0.5), cell([0.6, 0.2], 30, 0.8, 0.5)],
    };
    const g = gridLayout(two);
    expect([g.xs, g.ys, g.at.size, g.offGrid]).toEqual([[0.5, 0.6], [0.1, 0.2], 4, null]);
    expect(shadeStep(0.5, 0.5, 0.8, 6)).toBe(0);
    expect(shadeStep(0.8, 0.5, 0.8, 6)).toBe(5);
    expect(shadeStep(1, 1, 1, 6)).toBe(3);
  });
});

describe("SweepPanel", () => {
  it("one axis: a curve per quantity with the published value marked, and every cell in the table", () => {
    const { container } = render(<SweepPanel sweep={oneAxis} k={12} text={en} />);
    // Composite, green share and the chosen metric.
    expect(screen.getAllByTestId("sweep-curve")).toHaveLength(3);
    const marks = [...container.querySelectorAll("svg line > title")].filter((el) => el.textContent === en.sweep.publishedMarker);
    expect(marks).toHaveLength(3);
    const table = screen.getByRole("table", { name: en.analysis.tableTitle });
    const rows = within(table).getAllByRole("row").slice(1);
    expect(rows).toHaveLength(5);
    expect(rows[2]).toHaveTextContent(`0.65 (${en.analysis.publishedRow})`);
    // The suppressed cell shows "n < k", never 0.
    expect(rows[4]).toHaveTextContent("n < 12");
    expect(rows[4]).not.toHaveTextContent("0%");
    expect(screen.getByText(en.analysis.publishedValue("0.65"))).toBeInTheDocument();
  });

  it("two axes: the 625 grid cells of the seeded run in each heatmap, hidden cells grey, the off-grid published value listed", async () => {
    const run = await seeded(MOCK_RESEARCH_IDS.runSweep);
    const sweep = run.summary!.sweep!;
    expect(sweep.cells).toHaveLength(626);
    const { container } = render(<SweepPanel sweep={sweep} k={run.summary!.small_cell_k} text={en} />);
    expect(screen.getAllByTestId("sweep-heatmap")).toHaveLength(2);
    expect(screen.getAllByTestId("sweep-cell")).toHaveLength(1250);
    expect(screen.getByText(en.analysis.offGrid("0.65, 0.35"))).toBeInTheDocument();
    expect(screen.queryByTestId("sweep-published")).toBeNull();
    const grey = [...container.querySelectorAll('[data-testid="sweep-cell"]')].filter((el) => el.textContent?.includes("n < 12"));
    expect(grey).toHaveLength(8);
    expect(within(screen.getByRole("table", { name: en.analysis.tableTitle })).getAllByRole("row")).toHaveLength(627);
  });

  it("renders in Nepali from the same data", () => {
    render(<SweepPanel sweep={oneAxis} k={12} text={ne} />);
    expect(screen.getByRole("heading", { name: ne.analysis.sweepTitle })).toBeInTheDocument();
  });
});

describe("AgeProfileTable", () => {
  it("shows the shares under both baselines and hides the small band", async () => {
    const run = await seeded(MOCK_RESEARCH_IDS.runAgeProfile);
    const profile = parseAgeProfile(run.summary!.age_profile)!;
    expect(profile).not.toBeNull();
    render(<AgeProfileTable profile={profile} k={run.summary!.small_cell_k} text={en} />);
    const shown = screen.getByRole("region", { name: en.analysis.bandTitle("24-35", 24, 35) });
    const row = within(shown).getByTestId("age-row");
    expect(row).toHaveTextContent(en.analysis.shares("50%", "30%", "20%"));
    expect(row).toHaveTextContent(en.analysis.shares("30%", "50%", "20%"));
    expect(row).toHaveTextContent("0.25 to 0.5 (expected 0.38)");
    const hidden = screen.getByRole("region", { name: en.analysis.bandTitle("36-47", 36, 47) });
    expect(within(hidden).getByTestId("age-band-suppressed")).toHaveTextContent("n < 12");
    expect(within(hidden).queryByTestId("age-row")).toBeNull();
    expect(screen.getByText(en.analysis.baselines("2026.10.0", "2026.11.0-draft"))).toBeInTheDocument();
  });

  it("an unreadable block is absent, not guessed", () => {
    expect(parseAgeProfile(undefined)).toBeNull();
    expect(parseAgeProfile({ bands: "x" })).toBeNull();
  });
});

describe("mock seeds (RUI-7)", () => {
  it("the phase-B runs parse and carry their blocks", async () => {
    const api = createMockClient();
    const sweep = await api.getAnalysisRun(MOCK_RESEARCH_IDS.runSweep);
    expect(sweep.kind).toBe("param_sweep");
    expect((sweep.summary as RunSummary).sweep?.published_values).toEqual([0.65, 0.35]);
    const age = await api.getAnalysisRun(MOCK_RESEARCH_IDS.runAgeProfile);
    expect(age.kind).toBe("age_profile");
    expect(parseAgeProfile(age.summary?.age_profile)?.bands).toHaveLength(2);
  });
});
