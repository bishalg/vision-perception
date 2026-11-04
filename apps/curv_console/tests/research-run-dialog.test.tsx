/**
 * WP-X.9b: the run dialog starts sweeps and age profiles (RUI-5, SWP-1, SWP-2, AGE-1). The axis rules are pure
 * (`lib/research/run-sweep.ts`) and bounded by the registry row; the dialog shows the live cell count and the server's
 * problems at the field they name.
 */
import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { RunDialog, type RunStartInput } from "@/components/research/RunDialog";
import { createMockClient, MOCK_ORG_ID } from "@/lib/api/mock";
import { MOCK_RESEARCH_IDS } from "@/lib/api/mock/research";
import type { ParamSpec } from "@/lib/api/research";
import { paramRegistry } from "@/lib/research/registry";
import {
  axisKnobs,
  checkAxis,
  checkSweep,
  fieldTarget,
  MAX_SWEEP_CELLS,
  METRIC_KEYS,
  newAxis,
  sweepSpec,
  type AxisDraft,
} from "@/lib/research/run-sweep";
import { MAX_SWEEP_STEPS } from "@/lib/research/sweep";
import { paramText } from "@/lib/research/paramText";
import { researchMessages } from "@/messages/research";

const push = vi.fn();
vi.mock("next/navigation", () => ({ useRouter: () => ({ push, back: vi.fn(), refresh: vi.fn() }) }));
beforeEach(() => push.mockClear());

const en = researchMessages.en;
const ne = researchMessages.ne;
const spec = (id: string): ParamSpec => paramRegistry.params.find((p) => p.id === id)!;
const label = (id: string) => paramText("en", id).label;
const axis = (paramId: string, from: string, to: string, step: string): AxisDraft => ({ paramId, from, to, step });

const GREEN = "flag.greenFloor";
const YELLOW = "flag.yellowFloor";
const FRAMES_INT = "trackingQuality.calibration.minSamplesPerTarget";
const FRAMES_NUM = "qualityGate.minDistanceCm";
const CAPTURE = "trackingQuality.calibration.maxTargetWaitMs";
const OWNER_GATED = "trackingQuality.scoring.maxAccuracyDegForScoring";

describe("axis rules (SWP-1, SWP-2)", () => {
  it("offers Number and Integer knobs only, never capture_only, frames-stage knobs only at level frames", () => {
    const score = axisKnobs(paramRegistry.params, "auto").map((k) => k.id);
    const frames = axisKnobs(paramRegistry.params, "frames").map((k) => k.id);
    expect(score).toContain(GREEN);
    expect(score).toContain(OWNER_GATED);
    expect(score).not.toContain(FRAMES_NUM);
    expect(frames).toContain(FRAMES_NUM);
    expect(frames).toContain(FRAMES_INT);
    expect([...score, ...frames]).not.toContain(CAPTURE);
    for (const id of frames) expect(["number", "integer"]).toContain(spec(id).kind.type);
    expect(axisKnobs(paramRegistry.params, "score")).toEqual(axisKnobs(paramRegistry.params, "auto"));
  });

  it("walks the grid and adds the published value when it falls off the grid", () => {
    const on = checkAxis(spec(GREEN), axis(GREEN, "0.5", "0.8", "0.05"), "auto", {});
    expect(on).toMatchObject({ ok: true, steps: 6 });
    expect(on.ok && on.values).toHaveLength(7);
    const off = checkAxis(spec(GREEN), axis(GREEN, "0.5", "0.8", "0.1"), "auto", { flag: { greenFloor: 0.65 } });
    expect(off.ok && off.values).toEqual([0.5, 0.6, 0.65, 0.7, 0.8]);
    const outside = checkAxis(spec(GREEN), axis(GREEN, "0.7", "0.8", "0.1"), "auto", { flag: { greenFloor: 0.65 } });
    expect(outside.ok && outside.values).toEqual([0.7, 0.8]);
  });

  it("refuses what the registry row or the server would", () => {
    const reason = (s: ParamSpec | undefined, a: AxisDraft, level = "auto") => {
      const c = checkAxis(s, a, level, {});
      return c.ok ? "ok" : c.problem.reason;
    };
    expect(reason(spec(GREEN), axis(GREEN, "0.8", "0.5", "0.05"))).toBe("range");
    expect(reason(spec(GREEN), axis(GREEN, "", "0.5", "0.05"))).toBe("range");
    expect(reason(spec(GREEN), axis(GREEN, "0.1", "0.5", "0"))).toBe("step");
    expect(reason(spec(GREEN), axis(GREEN, "0.1", "0.5", "abc"))).toBe("step");
    expect(reason(spec(GREEN), axis(GREEN, "-0.1", "0.5", "0.05"))).toBe("bounds");
    expect(reason(spec(GREEN), axis(GREEN, "0.1", "1.5", "0.05"))).toBe("bounds");
    expect(reason(spec(GREEN), axis(GREEN, "0.1", "0.5", "0.001"))).toBe("fine");
    expect(reason(spec(GREEN), axis(GREEN, "0", "1", "0.01"))).toBe("too_many");
    expect(reason(spec(FRAMES_INT), axis(FRAMES_INT, "1.5", "10", "1"), "frames")).toBe("whole");
    expect(reason(spec(FRAMES_INT), axis(FRAMES_INT, "1", "10", "0.5"), "frames")).toBe("whole");
    expect(reason(spec(FRAMES_NUM), axis(FRAMES_NUM, "20", "40", "5"), "auto")).toBe("level");
    expect(reason(spec(CAPTURE), axis(CAPTURE, "0", "100", "10"), "frames")).toBe("param");
    expect(reason(spec("trackingQuality.provisional"), axis("trackingQuality.provisional", "0", "1", "1"))).toBe("param");
    expect(reason(undefined, axis("nope", "0", "1", "1"))).toBe("param");
    const limit = checkAxis(spec(GREEN), axis(GREEN, "0", "0.24", "0.01"), "auto", {});
    expect(limit).toMatchObject({ ok: true, steps: MAX_SWEEP_STEPS });
  });

  it("counts cells as the product of the axes and refuses a repeated knob or too many cells", () => {
    const two = checkSweep(paramRegistry.params, [axis(GREEN, "0.5", "0.7", "0.05"), axis(YELLOW, "0.2", "0.35", "0.05")], "auto", {});
    expect(two).toMatchObject({ ok: true, cells: 5 * 4, problem: null });
    expect(checkSweep(paramRegistry.params, [axis(GREEN, "0.5", "0.7", "0.05"), axis(GREEN, "0.2", "0.35", "0.05")], "auto", {})).toMatchObject({ ok: false, problem: "same_knob" });
    expect(checkSweep(paramRegistry.params, [], "auto", {})).toMatchObject({ ok: false, problem: "axes", cells: null });
    expect(checkSweep(paramRegistry.params, [axis(GREEN, "0.1", "0.2", "0.05"), axis(YELLOW, "0.1", "0.2", "0.05"), axis(GREEN, "0", "1", "0.5")], "auto", {})).toMatchObject({ problem: "axes" });
    expect(checkSweep(paramRegistry.params, [axis(GREEN, "0.5", "0.2", "0.05")], "auto", {})).toMatchObject({ ok: false, cells: null });
    // 25 x 25 is exactly the limit; the published value off the grid on one axis adds a column and goes over it.
    const limit = checkSweep(paramRegistry.params, [axis(GREEN, "0.5", "0.74", "0.01"), axis(YELLOW, "0", "0.24", "0.01")], "auto", {});
    expect(limit).toMatchObject({ ok: true, cells: MAX_SWEEP_CELLS });
    const over = checkSweep(paramRegistry.params, [axis(FRAMES_INT, "1", "49", "2"), axis(GREEN, "0.5", "0.74", "0.01")], "frames", {});
    expect(over).toMatchObject({ ok: false, cells: 26 * 25, problem: "cells" });
  });

  it("builds the request block from the typed bounds and routes server field names", () => {
    expect(sweepSpec([axis(GREEN, "0.5", "0.8", "0.05")])).toEqual({ axes: [{ param_id: GREEN, from: 0.5, to: 0.8, step: 0.05 }] });
    expect(newAxis(spec(GREEN))).toEqual({ paramId: GREEN, from: "0", to: "1", step: "0.05" });
    expect(fieldTarget("sweep.axes[1].step")).toEqual({ kind: "axis", index: 1 });
    expect(fieldTarget("sweep.axes[0]")).toEqual({ kind: "axis", index: 0 });
    expect(fieldTarget("sweep.axes")).toEqual({ kind: "sweep" });
    expect(fieldTarget("options.metrics[2]")).toEqual({ kind: "metrics" });
    expect(fieldTarget("cohort_id")).toEqual({ kind: "other" });
  });

  it("has every dialog string in both locales", () => {
    expect(Object.keys(ne.runSetup)).toEqual(Object.keys(en.runSetup));
    expect(Object.keys(ne.runSetup.problems)).toEqual(Object.keys(en.runSetup.problems));
    expect(Object.keys(ne.runSetup.kinds)).toEqual(["cohort_rescore", "param_sweep", "age_profile"]);
    expect(ne.runSetup.ageNote).not.toBe("");
  });
});

describe("RunDialog kind picker and axis editor", () => {
  const mock = createMockClient();

  async function open(runOnCohort: (input: RunStartInput) => ReturnType<React.ComponentProps<typeof RunDialog>["runOnCohort"]> = async () => ({ ok: true, runId: "r9" })) {
    const cohorts = (await mock.listCohorts(MOCK_ORG_ID)).items;
    const run = vi.fn(runOnCohort);
    render(<RunDialog orgId={MOCK_ORG_ID} experimentId={MOCK_RESEARCH_IDS.experimentDraft} cohorts={cohorts} dirty={false} previewCohort={vi.fn()} runOnCohort={run} onClose={() => undefined} />);
    const dialog = screen.getByRole("dialog", { name: en.run.title });
    await userEvent.selectOptions(within(dialog).getByLabelText(en.run.cohort), MOCK_RESEARCH_IDS.cohortToddlers);
    return { dialog, run };
  }
  const kind = (dialog: HTMLElement, k: keyof typeof en.runSetup.kinds) => userEvent.selectOptions(within(dialog).getByLabelText(en.runSetup.kind), k);

  it("defaults to the cohort re-score, with no axis editor", async () => {
    const { dialog, run } = await open();
    expect(within(dialog).getByLabelText(en.runSetup.kind)).toHaveValue("cohort_rescore");
    expect(within(dialog).queryByText(en.runSetup.axes)).not.toBeInTheDocument();
    await userEvent.click(within(dialog).getByRole("button", { name: en.run.submit }));
    await waitFor(() => expect(run).toHaveBeenCalledWith({ kind: "cohort_rescore", experiment_id: MOCK_RESEARCH_IDS.experimentDraft, cohort_id: MOCK_RESEARCH_IDS.cohortToddlers, level: "auto" }));
  });

  it("an age profile has no axes, says what it compares and starts without a sweep", async () => {
    const { dialog, run } = await open();
    await kind(dialog, "age_profile");
    expect(within(dialog).getByText(en.runSetup.ageNote)).toBeInTheDocument();
    expect(within(dialog).queryByRole("group", { name: en.runSetup.axis(1) })).not.toBeInTheDocument();
    await userEvent.click(within(dialog).getByRole("button", { name: en.run.submit }));
    await waitFor(() => expect(run).toHaveBeenCalledWith({ kind: "age_profile", experiment_id: MOCK_RESEARCH_IDS.experimentDraft, cohort_id: MOCK_RESEARCH_IDS.cohortToddlers, level: "auto" }));
    await waitFor(() => expect(push).toHaveBeenCalledWith(`/${MOCK_ORG_ID}/research/runs/r9`));
  });

  it("a sweep starts with one axis, a live cell count and the published value noted", async () => {
    const { dialog } = await open();
    await kind(dialog, "param_sweep");
    const first = within(dialog).getByRole("group", { name: en.runSetup.axis(1) });
    expect(within(first).getByLabelText(en.runSetup.knob)).toHaveValue(GREEN);
    const cells = checkSweep(paramRegistry.params, [newAxis(spec(GREEN))], "auto", {});
    expect(within(dialog).getByText(en.runSetup.cells(cells.cells!, MAX_SWEEP_CELLS))).toBeInTheDocument();
    expect(within(first).getByText(en.runSetup.published("0.65"))).toBeInTheDocument();
    // The inputs carry the registry row's bounds.
    expect(within(first).getByLabelText(en.runSetup.from)).toHaveAttribute("min", "0");
    expect(within(first).getByLabelText(en.runSetup.to)).toHaveAttribute("max", "1");
    // Two axes at most.
    await userEvent.click(within(dialog).getByRole("button", { name: en.runSetup.addAxis }));
    expect(within(dialog).getByRole("group", { name: en.runSetup.axis(2) })).toBeInTheDocument();
    expect(within(dialog).queryByRole("button", { name: en.runSetup.addAxis })).not.toBeInTheDocument();
    const both = checkSweep(paramRegistry.params, [newAxis(spec(GREEN)), newAxis(spec(YELLOW))], "auto", {});
    expect(within(dialog).getByText(en.runSetup.cells(both.cells!, MAX_SWEEP_CELLS))).toBeInTheDocument();
    await userEvent.click(within(dialog).getAllByRole("button", { name: en.runSetup.removeAxis })[1]!);
    expect(within(dialog).queryByRole("group", { name: en.runSetup.axis(2) })).not.toBeInTheDocument();
  });

  it("frames-stage knobs are offered only at level frames", async () => {
    const { dialog } = await open();
    await kind(dialog, "param_sweep");
    const first = within(dialog).getByRole("group", { name: en.runSetup.axis(1) });
    const options = () => within(first).getAllByRole("option").map((o) => o.textContent);
    expect(options()).not.toContain(label(FRAMES_NUM));
    expect(options()).toContain(label(OWNER_GATED));
    expect(within(dialog).getByText(en.runSetup.framesOnly)).toBeInTheDocument();
    await userEvent.selectOptions(within(dialog).getByLabelText(en.run.level), "frames");
    expect(options()).toContain(label(FRAMES_NUM));
    expect(within(dialog).queryByText(en.runSetup.framesOnly)).not.toBeInTheDocument();
    expect(options()).not.toContain(label(CAPTURE));
    await userEvent.selectOptions(within(first).getByLabelText(en.runSetup.knob), FRAMES_NUM);
    // Back below frames: the axis falls back to a knob the level allows.
    await userEvent.selectOptions(within(dialog).getByLabelText(en.run.level), "score");
    expect(within(first).getByLabelText(en.runSetup.knob)).toHaveValue(GREEN);
  });

  it("blocks the start and says why while an axis is out of bounds or too fine", async () => {
    const { dialog, run } = await open();
    await kind(dialog, "param_sweep");
    const first = within(dialog).getByRole("group", { name: en.runSetup.axis(1) });
    const submit = within(dialog).getByRole("button", { name: en.run.submit });
    expect(submit).toBeEnabled();
    fireEvent.change(within(first).getByLabelText(en.runSetup.step), { target: { value: "0.01" } });
    expect(within(first).getByRole("alert")).toHaveTextContent(en.runSetup.problems.tooMany(100, MAX_SWEEP_STEPS));
    expect(submit).toBeDisabled();
    fireEvent.change(within(first).getByLabelText(en.runSetup.step), { target: { value: "0.001" } });
    expect(within(first).getByRole("alert")).toHaveTextContent(en.runSetup.problems.fine(0.01));
    fireEvent.change(within(first).getByLabelText(en.runSetup.step), { target: { value: "0.1" } });
    fireEvent.change(within(first).getByLabelText(en.runSetup.to), { target: { value: "2" } });
    expect(within(first).getByRole("alert")).toHaveTextContent(en.runSetup.problems.bounds(0, 1));
    expect(within(dialog).getByText(en.runSetup.cellsUnknown)).toBeInTheDocument();
    expect(run).not.toHaveBeenCalled();
  });

  it("refuses two axes on the same knob", async () => {
    const { dialog } = await open();
    await kind(dialog, "param_sweep");
    await userEvent.click(within(dialog).getByRole("button", { name: en.runSetup.addAxis }));
    await userEvent.selectOptions(within(dialog).getByRole("group", { name: en.runSetup.axis(2) }).querySelector("select")!, GREEN);
    expect(within(dialog).getByText(en.runSetup.problems.sameKnob)).toBeInTheDocument();
    expect(within(dialog).getByRole("button", { name: en.run.submit })).toBeDisabled();
  });

  it("starts a sweep with the axes and the chosen metrics, then goes to the run page", async () => {
    const { dialog, run } = await open();
    await kind(dialog, "param_sweep");
    const first = within(dialog).getByRole("group", { name: en.runSetup.axis(1) });
    fireEvent.change(within(first).getByLabelText(en.runSetup.from), { target: { value: "0.5" } });
    fireEvent.change(within(first).getByLabelText(en.runSetup.to), { target: { value: "0.8" } });
    fireEvent.change(within(first).getByLabelText(en.runSetup.step), { target: { value: "0.1" } });
    expect(within(dialog).getAllByRole("checkbox")).toHaveLength(METRIC_KEYS.length);
    await userEvent.click(within(dialog).getByRole("checkbox", { name: "eye_dwell_ratio" }));
    await userEvent.click(within(dialog).getByRole("button", { name: en.run.submit }));
    await waitFor(() =>
      expect(run).toHaveBeenCalledWith({
        kind: "param_sweep",
        experiment_id: MOCK_RESEARCH_IDS.experimentDraft,
        cohort_id: MOCK_RESEARCH_IDS.cohortToddlers,
        level: "auto",
        sweep: { axes: [{ param_id: GREEN, from: 0.5, to: 0.8, step: 0.1 }] },
        options: { metrics: ["eye_dwell_ratio"] },
      }),
    );
    await waitFor(() => expect(push).toHaveBeenCalledWith(`/${MOCK_ORG_ID}/research/runs/r9`));
  });

  it("shows the server's validation problems at the axis and the metric that they name", async () => {
    const { dialog } = await open(async () => ({
      ok: false,
      slug: "validation-failed",
      status: 400,
      params: [
        { name: "sweep.axes[0].step", reason: "must be at least the registry step 0.01" },
        { name: "options.metrics[0]", reason: "not a metric key" },
      ],
    }));
    await kind(dialog, "param_sweep");
    await userEvent.click(within(dialog).getByRole("checkbox", { name: "eye_dwell_ratio" }));
    await userEvent.click(within(dialog).getByRole("button", { name: en.run.submit }));
    const first = within(dialog).getByRole("group", { name: en.runSetup.axis(1) });
    expect(await within(first).findByText("must be at least the registry step 0.01")).toBeInTheDocument();
    expect(within(within(dialog).getByRole("group", { name: en.runSetup.metrics })).getByText("not a metric key")).toBeInTheDocument();
    expect(push).not.toHaveBeenCalled();
    // Editing a field clears the server's reasons.
    fireEvent.change(within(first).getByLabelText(en.runSetup.step), { target: { value: "0.1" } });
    expect(within(first).queryByText("must be at least the registry step 0.01")).not.toBeInTheDocument();
  });

  it("shows sweep-too-large in words", async () => {
    const { dialog } = await open(async () => ({ ok: false, slug: "sweep-too-large", status: 400 }));
    await kind(dialog, "param_sweep");
    await userEvent.click(within(dialog).getByRole("button", { name: en.run.submit }));
    expect(await within(dialog).findByText(en.errors["sweep-too-large"]!)).toBeInTheDocument();
  });
});
