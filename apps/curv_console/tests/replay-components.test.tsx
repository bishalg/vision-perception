/**
 * WP-X.20 replay components: the trace chart's band, table toggle and CSV, the clip list's status words, the shared
 * playhead on the Quality tab, and the whole Replay tab against the real Rust core over the mock fixture (skipped
 * without the wasm build).
 */
import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";
import { ClipList } from "@/components/replay/ClipList";
import { SessionReplay } from "@/components/replay/SessionReplay";
import { TraceChart } from "@/components/replay/TraceChart";
import { setPlayhead } from "@/components/replay/replayStore";
import { QualityPanel } from "@/components/QualityPanel";
import { MOCK_ORG_ID, createMockClient } from "@/lib/api/mock";
import { ageMonthsOfRecord, framesOfRecord, type ExpectedBand, type ReplayTrace } from "@/lib/asd/replay";
import { HAS_WASM, nodeCore } from "./asdCoreNode";

beforeAll(() => {
  // jsdom has no 2D canvas; painting is skipped (the draw model is tested on its own).
  HTMLCanvasElement.prototype.getContext = (() => null) as unknown as HTMLCanvasElement["getContext"];
});
afterEach(() => setPlayhead(null));

const band: ExpectedBand = {
  segmentId: "full",
  startSec: 0,
  endSec: 2,
  metricKey: "aoi_dwell_ratio::Face",
  direction: "higherBetter",
  bracketId: "24-35",
  low: 0.35,
  high: null,
  mean: 0.62,
  thresholds: [
    { labelKey: "replay.band.expected", value: 0.62 },
    { labelKey: "replay.band.atypicalBelow", value: 0.35 },
  ],
  citation: "Synthetic",
  source: "literature",
};
const trace: ReplayTrace = {
  stimulusId: "c1",
  trace: {
    metric: { kind: "aoiDwell", aoi: "Face" },
    windowSec: 1,
    stepSec: 0.25,
    points: [0, 0.25, 0.5, 0.75].map((t, i) => ({ tSec: t, value: i === 2 ? null : 0.5 + i / 10, validRatio: 1, nFrames: 60 })),
  },
  bracketId: "24-35",
  bands: [band],
};

describe("TraceChart", () => {
  it("shades the band, labels the thresholds through the catalogue, toggles a table and downloads a CSV", async () => {
    const created: Blob[] = [];
    const createObjectURL = vi.fn((b: Blob) => {
      created.push(b);
      return "blob:x";
    });
    Object.assign(URL, { createObjectURL, revokeObjectURL: vi.fn() });
    const click = vi.spyOn(HTMLAnchorElement.prototype, "click").mockImplementation(() => undefined);
    render(<TraceChart clipId="c1" aoi="Face" trace={trace} start={0} end={2} tSec={0.5} />);
    const svg = screen.getByRole("img", { name: /Share of gaze on Face in a sliding window, 4 points/ });
    expect(within(svg).getByText("Expected 0.62")).toBeInTheDocument();
    expect(within(svg).getByText("Atypical below 0.35")).toBeInTheDocument();
    expect(svg.querySelectorAll('[data-testid="replay-bands"] rect')).toHaveLength(1);
    expect(screen.getByText("full: Expected range 0.35 and above")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Table" }));
    const table = screen.getByRole("table", { name: "Share of gaze on Face over the clip" });
    expect(within(table).getAllByRole("row")).toHaveLength(5);
    expect(within(table).getByText("no valid frame")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Chart" })).toHaveAttribute("aria-pressed", "true");

    fireEvent.click(screen.getByRole("button", { name: "Download CSV" }));
    expect(click).toHaveBeenCalledOnce();
    expect(await created[0]!.text()).toMatch(/^clip_id,metric,t_sec,value,valid_ratio,n_frames\r\nc1,aoiDwell:Face,0,0.5,1,60\r\n/);
    click.mockRestore();
  });

  it("says why there is no band", () => {
    const { rerender } = render(<TraceChart clipId="c1" aoi="Face" trace={{ ...trace, bands: [] }} start={0} end={2} tSec={0} />);
    expect(screen.getByText("No published expected range for this clip and age.")).toBeInTheDocument();
    rerender(<TraceChart clipId="c1" aoi="Face" trace={{ ...trace, bands: [] }} start={0} end={2} tSec={0} needAge />);
    expect(screen.getByText("Enter the child's age in months to draw the expected range.")).toBeInTheDocument();
  });
});

describe("ClipList", () => {
  it("prints the status word and the numbers, never colour alone, and selects a clip", () => {
    const onSelect = vi.fn();
    render(
      <ClipList
        selected="a"
        onSelect={onSelect}
        clips={[
          { id: "a", title: "Clip A", durationSec: 10, recorded: 0.58, band, status: "onTarget", aoi: "Face" },
          { id: "b", title: "Clip B", durationSec: 8, recorded: null, band: null, status: null, aoi: null },
        ]}
      />,
    );
    const items = screen.getAllByTestId("replay-clip");
    expect(items[0]).toHaveAttribute("aria-current", "true");
    expect(within(items[0]!).getByText("On target")).toBeInTheDocument();
    expect(within(items[0]!).getByText("Recorded 0.58")).toBeInTheDocument();
    expect(within(items[0]!).getByText("expected ≥ 0.35")).toBeInTheDocument();
    expect(within(items[1]!).getByText("Not scored")).toBeInTheDocument();
    expect(within(items[1]!).getByText("No expected range")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Show Clip B" }));
    expect(onSelect).toHaveBeenCalledWith("b");
  });
});

describe("QualityPanel playhead (V-4)", () => {
  it("shows the replay position of the same session only", async () => {
    const api = createMockClient();
    const { items } = await api.listSessions(MOCK_ORG_ID, { limit: 1 });
    const s = await api.getSession(items[0]!.id);
    render(<QualityPanel session={s} />);
    expect(screen.getByTestId("quality-playhead")).toHaveTextContent("Open the Replay tab to place a playhead.");
    act(() => setPlayhead({ sessionId: "other", clipId: "c", clipTitle: "Other", clipSec: 1, sessionSec: 1 }));
    expect(screen.getByTestId("quality-playhead")).toHaveTextContent("Open the Replay tab");
    act(() => setPlayhead({ sessionId: s.id, clipId: "c", clipTitle: "Clip A", clipSec: 2.5, sessionSec: 2.5 }));
    expect(screen.getByTestId("quality-playhead")).toHaveTextContent("Replay position: Clip A at 2.50 s");
  });
});

describe.skipIf(!HAS_WASM)("SessionReplay against the Rust core", () => {
  it("lists both clips with bands, draws the ribbon and the trace band, steps with the keyboard, switches clip", async () => {
    const api = createMockClient();
    const { items } = await api.listSessions(MOCK_ORG_ID, { limit: 1 });
    const id = items[0]!.id;
    const [raw, config, clips] = await Promise.all([api.getSessionRaw(id), api.getEffectiveConfig(MOCK_ORG_ID), api.getSessionClips(id)]);
    render(
      <SessionReplay
        sessionId={id}
        frames={framesOfRecord(raw.raw_payload)}
        ageMonths={ageMonthsOfRecord(raw.raw_payload)}
        catalog={config.stimulus_catalog.body}
        baselines={config.normative_baselines.body}
        profile={config.scoring_profile.body}
        clipStatuses={Object.fromEntries(clips.map((c) => [c.clip_id, c.status ?? null]))}
        core={nodeCore()}
      />,
    );
    await waitFor(() => expect(screen.getAllByTestId("replay-clip")).toHaveLength(2), { timeout: 10_000 });
    await waitFor(() => expect(screen.getAllByText(/^Recorded 0\.\d\d$/)).toHaveLength(2), { timeout: 10_000 });
    await waitFor(() => expect(document.querySelectorAll('[data-testid="replay-ribbon"] rect').length).toBeGreaterThan(3));
    await waitFor(() => expect(document.querySelectorAll('[data-testid="replay-bands"] rect').length).toBe(1));
    const canvas = screen.getByTestId("replay-canvas");
    expect(canvas).toHaveAttribute("role", "img");
    expect(canvas.getAttribute("aria-label")).toMatch(/^Gaze replay of Social versus geometric split screen at 0\.00 s, gaze /);
    fireEvent.keyDown(canvas, { key: "ArrowRight" });
    expect(canvas.getAttribute("data-time")).toBe("0.017");
    fireEvent.keyDown(canvas, { key: "ArrowLeft" });
    expect(canvas.getAttribute("data-time")).toBe("0.000");
    expect(screen.getByText("No cohort heatmap: this session is not linked to an analysis run.")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Show Gaze cue to a toy" }));
    await waitFor(() => expect(screen.getByTestId("replay-canvas")).toHaveAttribute("data-clip", "stim_02_gaze_cue_8s"));
    await waitFor(() => expect(document.querySelectorAll('[data-testid="replay-bands"] rect').length).toBe(2));
  });

  it("says so when the record has no frames", () => {
    render(<SessionReplay sessionId="x" frames={null} ageMonths={null} catalog={null} baselines={null} profile={null} clipStatuses={{}} />);
    expect(screen.getByText("This stored record has no gaze frames, so there is nothing to replay.")).toBeInTheDocument();
  });
});
