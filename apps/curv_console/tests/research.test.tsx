/**
 * WP-X.5: the research area. The registry form is generated from the parameter registry (PRM-6, RUI-2), the preview
 * re-scores through the Rust core (here a stand-in with the asd-wasm envelopes, `./researchStub`), the sweep and
 * debounce are pure. Frame-level re-processing is `unavailable` until WP-X.4.
 */
import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ParamControl } from "@/components/research/ParamControl";
import { PreviewPanel } from "@/components/research/PreviewPanel";
import { RegistryForm } from "@/components/research/RegistryForm";
import { RunDialog } from "@/components/research/RunDialog";
import { ScanpathPlot } from "@/components/research/ScanpathPlot";
import { SweepCurve } from "@/components/research/SweepCurve";
import { Workbench, type WorkbenchActions } from "@/components/research/Workbench";
import { createMockClient, MOCK_ORG_ID } from "@/lib/api/mock";
import { MOCK_RESEARCH_IDS } from "@/lib/api/mock/research";
import type { ParamSpec } from "@/lib/api/research";
import { AsdCore, type AsdWasmExports } from "@/lib/asd/core";
import { paramRegistry as wasmParamRegistry, reprocessSession } from "@/lib/asd/ops";
import { navItems } from "@/lib/nav";
import { MAX_PREVIEW_SESSIONS, PREVIEW_TRACE_HZ, PREVIEW_DEBOUNCE_MS, debounce, paramsNotApplied, parseTrace, pickerSessions } from "@/lib/research/preview";
import { clampParam, diffParams, getAt, groupOf, groupRegistry, paramRegistry, setAt, valueOf } from "@/lib/research/registry";
import { MAX_SWEEP_VALUES, defaultSweep, sweepValues } from "@/lib/research/sweep";
import { LocaleProvider } from "@/lib/i18n/provider";
import { paramText } from "@/lib/research/paramText";
import { researchMessages, researchText } from "@/messages/research";
import { stubCore, stubModule } from "./researchStub";

// The registry form has 88 controls; typing into it under jsdom is slow.
vi.setConfig({ testTimeout: 30_000 });

const push = vi.fn();
vi.mock("next/navigation", () => ({ useRouter: () => ({ push, back: vi.fn(), refresh: vi.fn() }) }));
vi.mock("@/lib/asd/browser", async () => {
  const { stubCore: make } = await import("./researchStub");
  const core = make();
  return { browserCore: () => core, ASD_WASM_BASE: "/asd-wasm/" };
});

const en = researchMessages.en;
const ne = researchMessages.ne;
const spec = (id: string): ParamSpec => paramRegistry.params.find((p) => p.id === id)!;
const label = (id: string) => paramText("en", id).label;
const defaults = { schemaVersion: "1.2.0", flag: { greenFloor: 0.65, yellowFloor: 0.35, lowConfidenceShare: 0.5, atypicalShareForRed: 0.4 }, lowConfidenceValidRatio: 0.6 };

beforeEach(() => push.mockClear());
afterEach(() => vi.useRealTimers());

describe("registry helpers", () => {
  it("groups by first path segment in registry order, a root-level knob is its own group", () => {
    const groups = groupRegistry(paramRegistry.params);
    expect(groups.map((g) => g.group)).toEqual(["flag", "lowConfidenceValidRatio", "qualityGate", "trackingQuality", "metrics", "aoi", "ageBands", "norms", "review", "display", "sessionQuality"]);
    expect(groups.flatMap((g) => g.params.map((p) => p.id)).sort()).toEqual(paramRegistry.params.map((p) => p.id).sort());
    expect(groupOf("lowConfidenceValidRatio")).toBe("lowConfidenceValidRatio");
    expect(groupOf("flag.greenFloor")).toBe("flag");
  });

  it("reads and writes a dotted path without touching the source", () => {
    const doc = { flag: { greenFloor: 0.65 } };
    const next = setAt(doc, "trackingQuality.calibration.settleMs", 100);
    expect(getAt(next, "trackingQuality.calibration.settleMs")).toBe(100);
    expect(getAt(doc, "trackingQuality")).toBeUndefined();
    expect(valueOf({}, spec("flag.greenFloor"))).toBe(0.65);
  });

  it("counts the knobs that differ from the published profile", () => {
    const a = defaults;
    const b = setAt(setAt(a, "flag.greenFloor", 0.7), "aoi.marginNorm", 0.05);
    expect(diffParams(paramRegistry.params, a, b)).toEqual(["flag.greenFloor", "aoi.marginNorm"]);
    // A value equal to the registry default is not a difference from a profile that omits it.
    expect(diffParams(paramRegistry.params, a, setAt(a, "aoi.marginNorm", 0))).toEqual([]);
  });

  it("clamps to min, max and step", () => {
    const k = spec("flag.greenFloor").kind;
    expect(clampParam(k, 1.5)).toBe(1);
    expect(clampParam(k, -3)).toBe(0);
    expect(clampParam(k, 0.123)).toBe(0.12);
    expect(clampParam(spec("aoi.marginNorm").kind, 0.0123)).toBe(0.01);
    expect(clampParam(spec("aoi.dwellCapMs").kind, 7.4)).toBe(16);
    expect(clampParam(k, Number.NaN)).toBeNull();
  });
});

describe("RegistryForm", () => {
  const form = (props: Partial<React.ComponentProps<typeof RegistryForm>> = {}) => (
    <RegistryForm registry={paramRegistry} published={defaults} draft={defaults} context="edit" onChange={() => undefined} {...props} />
  );

  it("renders one control per registry row, in registry order, with label, help and badge from the catalogue", () => {
    const { container } = render(form());
    const ids = [...container.querySelectorAll("[data-param-id]")].map((n) => n.getAttribute("data-param-id"));
    expect(ids).toEqual(paramRegistry.params.map((p) => p.id));
    const first = container.querySelector('[data-param-id="flag.greenFloor"]') as HTMLElement;
    expect(within(first).getByText(label("flag.greenFloor"))).toBeInTheDocument();
    expect(within(first).getByText(paramText("en", "flag.greenFloor").help)).toBeInTheDocument();
    expect(within(first).getByText(en.reprocess.score)).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: en.groups.metrics })).toBeInTheDocument();
  });

  it("disables capture_only knobs in the preview context and enables them when editing", () => {
    const capture = paramRegistry.params.filter((p) => p.reprocess === "capture_only");
    expect(capture.length).toBeGreaterThan(0);
    const { container, unmount } = render(form({ context: "preview" }));
    for (const p of capture) {
      const row = container.querySelector(`[data-param-id="${p.id}"]`) as HTMLElement;
      expect(within(row).getByText(en.form.captureOnlyPreview)).toBeInTheDocument();
      for (const c of row.querySelectorAll("input, select, button")) expect(c).toBeDisabled();
    }
    const live = container.querySelector('[data-param-id="flag.greenFloor"]') as HTMLElement;
    expect(within(live).getByLabelText(label("flag.greenFloor"))).toBeEnabled();
    unmount();
    const edit = render(form({ context: "edit" }));
    const row = edit.container.querySelector(`[data-param-id="${capture[0]!.id}"]`) as HTMLElement;
    expect(within(row).queryByText(en.form.captureOnlyPreview)).toBeNull();
  });

  it("shows the diff count against the published profile and reports the next draft", async () => {
    const onChange = vi.fn();
    const { rerender } = render(form({ onChange }));
    expect(screen.getByRole("status")).toHaveTextContent(en.form.diffCount(0));
    const green = screen.getByLabelText(label("flag.greenFloor"));
    await userEvent.clear(green);
    await userEvent.type(green, "0.7");
    expect(onChange).toHaveBeenLastCalledWith(expect.objectContaining({ flag: expect.objectContaining({ greenFloor: 0.7 }) }));
    rerender(form({ draft: setAt(defaults, "flag.greenFloor", 0.7), onChange }));
    expect(screen.getByRole("status")).toHaveTextContent(en.form.diffCount(1));
    expect(screen.getByText(en.form.publishedValue("0.65"))).toBeInTheDocument();
  });

  it("shows the Nepali label for the same knob (ADR-0013)", () => {
    render(<LocaleProvider locale="ne">{form({ text: ne })}</LocaleProvider>);
    expect(screen.getByLabelText(paramText("ne", "flag.greenFloor").label)).toBeInTheDocument();
    expect(paramText("ne", "flag.greenFloor").label).not.toBe(label("flag.greenFloor"));
  });

  it("falls back to the id when a catalogue key is missing", () => {
    expect(paramText("en", { id: "flag.greenFloor", labelKey: "param.nope.label", helpKey: "param.nope.help" })).toEqual({
      label: "flag.greenFloor",
      help: "flag.greenFloor",
    });
    expect(paramText("en", "no.such.row")).toEqual({ label: "no.such.row", help: "no.such.row" });
  });

  it("renders a control per kind: bool, enum, point list and age bands", () => {
    render(form());
    expect(screen.getByLabelText(label("trackingQuality.calibration.autoSelect"))).toHaveAttribute("type", "checkbox");
    expect(screen.getByLabelText(label("metrics.ivtUnit")).tagName).toBe("SELECT");
    expect(screen.getAllByRole("group", { name: en.form.pointsOf(label("trackingQuality.calibration.layout5"), 1) })).toHaveLength(1);
    expect(screen.getAllByRole("button", { name: en.form.addBand })).toHaveLength(1);
  });
});

describe("ParamControl", () => {
  it("clamps the slider to min, max and step and marks the default with a reset link", () => {
    const onChange = vi.fn();
    const s = spec("flag.greenFloor");
    const { rerender } = render(<ParamControl spec={s} value={0.65} published={0.65} context="edit" onChange={onChange} />);
    expect(screen.getByText(en.form.defaultValue("0.65"))).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: en.form.reset })).toBeNull();
    const slider = screen.getByLabelText(en.form.slider(label("flag.greenFloor")));
    expect(slider).toHaveAttribute("min", "0");
    expect(slider).toHaveAttribute("max", "1");
    expect(slider).toHaveAttribute("step", "0.01");
    fireEvent.change(slider, { target: { value: "7" } });
    expect(onChange).toHaveBeenLastCalledWith(1);
    fireEvent.change(slider, { target: { value: "0.333" } });
    expect(onChange).toHaveBeenLastCalledWith(0.33);
    rerender(<ParamControl spec={s} value={0.9} published={0.65} context="edit" onChange={onChange} />);
    fireEvent.click(screen.getByRole("button", { name: en.form.reset }));
    expect(onChange).toHaveBeenLastCalledWith(0.65);
  });

  it("keeps an out-of-range typed value so the core can name the problem, and flags it", async () => {
    const onChange = vi.fn();
    render(<ParamControl spec={spec("flag.greenFloor")} value={0.65} published={0.65} context="edit" onChange={onChange} error="flag.greenFloor must be in [0, 1]" />);
    const input = screen.getByLabelText(label("flag.greenFloor"));
    fireEvent.change(input, { target: { value: "1.5" } });
    expect(onChange).toHaveBeenLastCalledWith(1.5);
    expect(input).toHaveAttribute("aria-invalid", "true");
    expect(screen.getByText("flag.greenFloor must be in [0, 1]")).toBeInTheDocument();
  });

  it("edits a point list within its limits", async () => {
    const onChange = vi.fn();
    const s = spec("trackingQuality.validation.layout");
    render(<ParamControl spec={s} value={s.default} published={s.default} context="edit" onChange={onChange} />);
    await userEvent.click(screen.getByRole("button", { name: en.form.addPoint }));
    expect(onChange).toHaveBeenLastCalledWith([...(s.default as number[][]), [0, 0]]);
  });
});

describe("sweep stepping", () => {
  const kind = spec("flag.greenFloor").kind;

  it("produces at most 25 values and includes the published one", () => {
    const r = sweepValues(kind, 0.5, 0.8, 0.05, 0.65);
    expect(r).toEqual({ ok: true, values: [0.5, 0.55, 0.6, 0.65, 0.7, 0.75, 0.8] });
    const off = sweepValues(kind, 0.5, 0.8, 0.1, 0.65);
    expect(off.ok && off.values).toEqual([0.5, 0.6, 0.65, 0.7, 0.8]);
    const wide = sweepValues(kind, 0, 1, 0.05, 0.65);
    expect(wide.ok && wide.values.length).toBeLessThanOrEqual(MAX_SWEEP_VALUES);
    const dense = sweepValues(kind, 0, 0.96, 0.04, 0.6);
    expect(dense.ok && dense.values.length).toBeLessThanOrEqual(25);
    expect(dense.ok && dense.values).toContain(0.6);
  });

  it("refuses more than 24 steps and a bad range, and never leaves the registry limits", () => {
    expect(sweepValues(kind, 0, 1, 0.01, 0.65)).toEqual({ ok: false, reason: "too_many" });
    expect(sweepValues(kind, 0.8, 0.5, 0.05, 0.65)).toEqual({ ok: false, reason: "range" });
    expect(sweepValues(kind, 0.5, 0.8, 0, 0.65)).toEqual({ ok: false, reason: "step" });
    const r = sweepValues(kind, 0.9, 1.4, 0.1, 0.65);
    expect(r.ok && Math.max(...r.values)).toBeLessThanOrEqual(1);
  });

  it("starts from the whole registry range in at most 24 steps for every sweepable knob", () => {
    for (const p of paramRegistry.params.filter((x) => x.kind.type === "number" || x.kind.type === "integer")) {
      const d = defaultSweep(p.kind)!;
      expect(sweepValues(p.kind, d.from, d.to, d.step, d.from).ok, p.id).toBe(true);
    }
  });
});

describe("preview helpers", () => {
  it("debounces to one call per 150 ms of quiet, with the last arguments", () => {
    vi.useFakeTimers();
    const fn = vi.fn();
    const d = debounce(fn);
    for (let i = 0; i < 5; i += 1) {
      d(i);
      vi.advanceTimersByTime(PREVIEW_DEBOUNCE_MS - 50);
    }
    expect(fn).not.toHaveBeenCalled();
    vi.advanceTimersByTime(50);
    expect(fn).toHaveBeenCalledTimes(1);
    expect(fn).toHaveBeenCalledWith(4);
    d(5);
    d.cancel();
    vi.advanceTimersByTime(1000);
    expect(fn).toHaveBeenCalledTimes(1);
    expect(PREVIEW_DEBOUNCE_MS).toBe(150);
  });

  it("names the changed knobs the level cannot apply", () => {
    const draft = setAt(setAt(setAt(defaults, "flag.greenFloor", 0.7), "metrics.ivtVelocityUnitsPerSec", 4), "trackingQuality.calibration.maxRetriesPerTarget", 3);
    expect(paramsNotApplied(paramRegistry.params, defaults, draft, "score")).toEqual(["trackingQuality.calibration.maxRetriesPerTarget", "metrics.ivtVelocityUnitsPerSec"].sort((a, b) => paramRegistry.params.findIndex((p) => p.id === a) - paramRegistry.params.findIndex((p) => p.id === b)));
    expect(paramsNotApplied(paramRegistry.params, defaults, draft, "frames")).toEqual(["trackingQuality.calibration.maxRetriesPerTarget"]);
  });

  it("reads the core Evaluation: trace array and top-level fixations", () => {
    const evaluation = {
      level: "frames",
      trace: [
        { tSec: 0, x: 0.1, y: 0.2, state: "tracked", aoi: "face" },
        { tSec: 0.1, x: 0.3, y: 0.4, state: "tracked", fixationId: 0 },
      ],
      fixations: [{ stimulusId: "c1", startSec: 0, endSec: 0.2, durationMs: 200, xNorm: 0.2, yNorm: 0.3, firstFrame: 0, endFrame: 6 }],
    };
    expect(parseTrace(evaluation)).toEqual({ points: [{ x: 0.1, y: 0.2 }, { x: 0.3, y: 0.4 }], fixations: [{ x: 0.2, y: 0.3 }] });
    expect(parseTrace({ trace: evaluation.trace })?.fixations).toEqual([]);
  });

  it("returns null without a usable trace", () => {
    expect(parseTrace({ level: "score" })).toBeNull();
    expect(parseTrace({ trace: [] })).toBeNull();
    expect(parseTrace({ trace: [{ x: "a" }] })).toBeNull();
    expect(parseTrace({ trace: { points: [{ x: 0, y: 0 }] } })).toBeNull();
    expect(parseTrace(null)).toBeNull();
  });

  it("takes the picker frames flag from the session list item", () => {
    const item = { id: "s1", participant_token: "P1", started_at: "2026-11-01T00:00:00Z" };
    const rows = pickerSessions([
      { ...item, research_frames_available: true },
      { ...item, id: "s2", research_frames_available: false },
    ] as never);
    expect(rows.map((r) => [r.id, r.token, r.framesAvailable])).toEqual([["s1", "P1", true], ["s2", "P1", false]]);
  });

});

describe("core wrappers", () => {
  it("reprocessSession and paramRegistry answer unavailable until the core exports them (WP-X.4)", async () => {
    const core = stubCore();
    const a = await reprocessSession(core, "{}\n", {}, {}, {}, { ageMonths: 30 });
    expect(a.ok).toBe(false);
    if (!a.ok) expect(a.error.kind).toBe("unavailable");
    const b = await wasmParamRegistry(core);
    expect(b.ok).toBe(false);
  });

  it("decodes both when the core has them", async () => {
    const mod = {
      ...stubModule(),
      paramRegistry: () => JSON.stringify({ ok: true, result: { registryVersion: "1.0.0", params: [] } }),
      reprocessSession: () => JSON.stringify({ ok: true, result: { trace: { points: [] } } }),
    } as unknown as AsdWasmExports;
    const core = new AsdCore(async () => mod);
    const reg = await wasmParamRegistry(core);
    expect(reg.ok && reg.result.registryVersion).toBe("1.0.0");
    const re = await reprocessSession(core, "{}\n", {}, {}, {}, { ageMonths: 30 });
    expect(re.ok).toBe(true);
  });
});

describe("charts", () => {
  it("draws the scanpath as plain SVG with both traces, fixations and areas (no script, no style attribute)", () => {
    const trace = { points: [{ x: 0.1, y: 0.1 }, { x: 0.5, y: 0.5 }], fixations: [{ x: 0.5, y: 0.5 }] };
    const { container } = render(<ScanpathPlot text={researchText} published={trace} draft={trace} aois={[{ name: "eyes", xMin: 0.4, yMin: 0.4, xMax: 0.6, yMax: 0.6 }]} />);
    const svg = screen.getByRole("img", { name: en.preview.scanpathLabel });
    expect(svg.querySelectorAll("polyline")).toHaveLength(2);
    expect(svg.querySelectorAll("circle")).toHaveLength(2);
    expect(svg.querySelectorAll("rect").length).toBe(2);
    expect(container.querySelector("script, [style]")).toBeNull();
  });

  it("marks the published value on the sweep curve", () => {
    const { container } = render(<SweepCurve text={researchText} points={[{ x: 0.5, y: 0.5 }, { x: 0.65, y: 0.5 }, { x: 0.8, y: null }]} published={0.65} label="Green floor" />);
    expect(screen.getByRole("img", { name: en.sweep.curve("Green floor") })).toBeInTheDocument();
    expect(container.querySelectorAll("line").length).toBe(3);
    expect(container.querySelector("script, [style]")).toBeNull();
  });
});

const sessions = [
  { id: "s1", token: "tok-1", startedAt: "2026-10-01T10:00:00Z", framesAvailable: true },
  { id: "s2", token: "tok-2", startedAt: "2026-10-02T10:00:00Z", framesAvailable: false },
];
const okLoad = vi.fn(async (ids: string[]) => ({ ok: true as const, records: ids.map((id) => ({ id, record: { id } })) }));
const noFrames = vi.fn(async () => ({ ok: false as const, slug: "not-implemented", status: 501 }));

describe("PreviewPanel", () => {
  beforeEach(() => {
    okLoad.mockClear();
    noFrames.mockClear();
  });

  it("re-scores published and draft; the published column never moves; frame logs are read once; frames fall back to the score level", async () => {
    const draft0 = setAt(defaults, "flag.greenFloor", 0.65);
    const props = { registry: paramRegistry, publishedProfile: defaults, baselines: {}, sessions, loadRecords: okLoad, loadFrames: noFrames };
    const { rerender } = render(<PreviewPanel {...props} draft={draft0} />);
    const card1 = await screen.findByRole("article", { name: en.preview.sessionCard("s1") }, { timeout: 3000 });
    await waitFor(() => expect(within(card1).getAllByLabelText(en.preview.composite)).toHaveLength(2));
    const values = () => within(card1).getAllByLabelText(en.preview.composite).map((n) => n.textContent);
    expect(values()).toEqual(["0.650", "0.650"]);
    expect(card1).toHaveAttribute("data-changed", "false");

    rerender(<PreviewPanel {...props} draft={setAt(defaults, "flag.greenFloor", 0.95)} />);
    await waitFor(() => expect(values()).toEqual(["0.650", "0.950"]), { timeout: 3000 });
    expect(within(card1).getByText(en.preview.flagLabel.red, { exact: false })).toBeInTheDocument();
    expect(card1).toHaveAttribute("data-changed", "true");

    // Records and the research log were each read once, however often the knob moved.
    expect(okLoad).toHaveBeenCalledTimes(1);
    expect(noFrames).toHaveBeenCalledTimes(1);
    expect(noFrames).toHaveBeenCalledWith("s1");
    expect(within(card1).getByText(en.preview.framesUnavailable)).toBeInTheDocument();
    expect(within(card1).getByTestId("scanpath")).toBeInTheDocument();
    const card2 = screen.getByRole("article", { name: en.preview.sessionCard("s2") });
    expect(within(card2).queryByTestId("scanpath")).toBeNull();
  });

  it("frames level: passes traceHz, parses the core Evaluation and draws the scanpath", async () => {
    const seen: unknown[] = [];
    const mod = {
      ...stubModule(),
      reprocessSession: (...a: unknown[]) => {
        seen.push(JSON.parse(String(a[4])));
        return JSON.stringify({
          ok: true,
          result: { level: "frames", trace: [{ tSec: 0, x: 0.1, y: 0.2, state: "tracked" }, { tSec: 0.1, x: 0.4, y: 0.5, state: "tracked" }], fixations: [{ xNorm: 0.4, yNorm: 0.5 }] },
        });
      },
    } as unknown as AsdWasmExports;
    const core = new AsdCore(async () => mod);
    const loadFrames = vi.fn(async () => ({ ok: true as const, jsonl: "{}\n" }));
    render(<PreviewPanel registry={paramRegistry} publishedProfile={defaults} baselines={{}} draft={defaults} sessions={sessions} loadRecords={okLoad} loadFrames={loadFrames} core={core} />);
    const card1 = await screen.findByRole("article", { name: en.preview.sessionCard("s1") }, { timeout: 3000 });
    await waitFor(() => expect(within(card1).getByRole("img", { name: en.preview.scanpathLabel })).toBeInTheDocument(), { timeout: 3000 });
    expect(within(card1).queryByText(en.preview.framesUnavailable)).toBeNull();
    expect(seen.length).toBeGreaterThan(0);
    expect(seen[0]).toMatchObject({ traceHz: PREVIEW_TRACE_HZ });
  });

  it("flags a changed knob the score level cannot apply", async () => {
    const draft = setAt(defaults, "metrics.ivtVelocityUnitsPerSec", 4);
    render(<PreviewPanel registry={paramRegistry} publishedProfile={defaults} baselines={{}} draft={draft} sessions={sessions} loadRecords={okLoad} loadFrames={noFrames} />);
    const card = await screen.findByRole("article", { name: en.preview.sessionCard("s2") }, { timeout: 3000 });
    expect(within(card).getByText(en.preview.paramNotApplied("metrics.ivtVelocityUnitsPerSec"))).toBeInTheDocument();
  });

  it("limits the selection to ten sessions", () => {
    const many = Array.from({ length: 12 }, (_, i) => ({ id: `m${i}`, token: `t${i}`, startedAt: "2026-10-01T10:00:00Z", framesAvailable: false }));
    render(<PreviewPanel registry={paramRegistry} publishedProfile={defaults} baselines={null} draft={defaults} sessions={many} loadRecords={okLoad} loadFrames={noFrames} />);
    expect(screen.getByText(en.preview.noBaselines)).toBeInTheDocument();
    expect(MAX_PREVIEW_SESSIONS).toBe(10);
  });

  it("sweeps one knob on one session without a job: the curve shows the composite against the value", async () => {
    render(<PreviewPanel registry={paramRegistry} publishedProfile={defaults} baselines={{}} draft={defaults} sessions={sessions} loadRecords={okLoad} loadFrames={noFrames} />);
    await screen.findByRole("article", { name: en.preview.sessionCard("s2") }, { timeout: 3000 });
    await userEvent.selectOptions(screen.getByLabelText(en.sweep.knob), "flag.greenFloor");
    fireEvent.change(screen.getByLabelText(en.sweep.from), { target: { value: "0.5" } });
    fireEvent.change(screen.getByLabelText(en.sweep.to), { target: { value: "0.8" } });
    fireEvent.change(screen.getByLabelText(en.sweep.step), { target: { value: "0.05" } });
    await userEvent.click(screen.getByRole("button", { name: en.sweep.run }));
    const table = await screen.findByRole("table", { name: en.sweep.curve(label("flag.greenFloor")) }, { timeout: 3000 });
    expect(within(table).getAllByRole("row")).toHaveLength(8);
    expect(screen.getByTestId("sweep-curve")).toBeInTheDocument();
    fireEvent.change(screen.getByLabelText(en.sweep.step), { target: { value: "0.001" } });
    await userEvent.click(screen.getByRole("button", { name: en.sweep.run }));
    expect(await screen.findByText(en.sweep.tooMany(24))).toBeInTheDocument();
  });
});

describe("Workbench", () => {
  const mock = createMockClient();
  const actions = (over: Partial<WorkbenchActions> = {}): WorkbenchActions => ({
    update: vi.fn(async (rowVersion) => ({ ok: true as const, rowVersion: rowVersion + 1, profileHash: "h" })),
    promote: vi.fn(async (i) => ({ ok: true as const, version: i.version })),
    previewCohort: vi.fn(),
    runOnCohort: vi.fn(async () => ({ ok: true as const, runId: "r1" })),
    loadRecords: okLoad,
    loadFrames: noFrames,
    ...over,
  });

  async function setup(over: Partial<WorkbenchActions> = {}, flags: { edit?: boolean } = {}) {
    const experiment = await mock.getExperiment(MOCK_RESEARCH_IDS.experimentDraft);
    const cohorts = (await mock.listCohorts(MOCK_ORG_ID)).items;
    const a = actions(over);
    render(
      <Workbench
        orgId={MOCK_ORG_ID}
        experiment={experiment}
        publishedProfile={defaults}
        baselines={{}}
        sessions={sessions}
        cohorts={cohorts}
        canEdit={flags.edit ?? true}
        canRun
        canPromote
        actions={a}
      />,
    );
    return { a, experiment, cohorts };
  }

  it("shows the Rust core's message at the knob and blocks saving until it is fixed (acceptance 3)", async () => {
    const { a } = await setup();
    const save = screen.getByRole("button", { name: en.workbench.save });
    expect(save).toBeDisabled();
    const green = screen.getByLabelText(label("flag.greenFloor"));
    await userEvent.clear(green);
    await userEvent.type(green, "1.5");
    expect(await screen.findAllByText("flag.greenFloor must be in [0, 1]", {}, { timeout: 3000 })).not.toHaveLength(0);
    expect(green).toHaveAttribute("aria-invalid", "true");
    expect(save).toBeDisabled();
    await userEvent.clear(green);
    await userEvent.type(green, "0.7");
    await waitFor(() => expect(save).toBeEnabled(), { timeout: 3000 });
    await userEvent.click(save);
    await waitFor(() => expect(a.update).toHaveBeenCalled());
    expect(a.update).toHaveBeenCalledWith(1, expect.objectContaining({ name: "Lower I-VT threshold", profile: expect.objectContaining({ flag: expect.objectContaining({ greenFloor: 0.7 }) }) }));
    expect(await screen.findByText(en.workbench.saved)).toBeInTheDocument();
    expect(save).toBeDisabled();
  });

  it("shows the server's field problems and a stale-version message", async () => {
    await setup({ update: vi.fn(async () => ({ ok: false as const, slug: "stale-version", status: 412, params: [{ name: "flag.yellowFloor", reason: "must be below greenFloor" }] })) });
    await userEvent.type(screen.getByLabelText(en.workbench.nameLabel), " v2");
    await userEvent.click(screen.getByRole("button", { name: en.workbench.save }));
    expect(await screen.findByText(en.errors["stale-version"]!, { exact: false })).toBeInTheDocument();
    expect(screen.getAllByText("must be below greenFloor").length).toBeGreaterThan(0);
  });

  it("is read-only for a role that cannot edit", async () => {
    await setup({}, { edit: false });
    expect(screen.getByLabelText(en.workbench.nameLabel)).toBeDisabled();
    expect(screen.getByText(en.workbench.readOnly)).toBeInTheDocument();
  });

  it("runs on a cohort: preview counts with suppressed bands, then the run page (RUI-5)", async () => {
    const preview = await mock.previewCohort(MOCK_ORG_ID, { age_bands: ["24-35"] });
    const { a } = await setup({ previewCohort: vi.fn(async () => ({ ok: true as const, preview: { ...preview, n: null, by_age_band: [{ band: "36-47", n: null, frames_n: null, suppressed: true }] } })) });
    await userEvent.click(screen.getByRole("button", { name: en.workbench.runOnCohort }));
    const dialog = screen.getByRole("dialog", { name: en.run.title });
    await userEvent.selectOptions(within(dialog).getByLabelText(en.run.cohort), MOCK_RESEARCH_IDS.cohortToddlers);
    await userEvent.click(within(dialog).getByRole("button", { name: en.run.preview }));
    expect(await within(dialog).findByText(en.run.bandRow("36-47", en.suppressedCell(preview.small_cell_k)))).toBeInTheDocument();
    await userEvent.type(within(dialog).getByLabelText(en.run.seed), "7");
    await userEvent.click(within(dialog).getByRole("button", { name: en.run.submit }));
    await waitFor(() => expect(a.runOnCohort).toHaveBeenCalledWith({ kind: "cohort_rescore", experiment_id: MOCK_RESEARCH_IDS.experimentDraft, cohort_id: MOCK_RESEARCH_IDS.cohortToddlers, level: "auto", seed: 7 }));
    await waitFor(() => expect(push).toHaveBeenCalledWith(`/${MOCK_ORG_ID}/research/runs/r1`));
  });

  it("blocks the run while there are unsaved changes", async () => {
    const experiment = await mock.getExperiment(MOCK_RESEARCH_IDS.experimentDraft);
    const preview = vi.fn();
    render(<RunDialog orgId="o" experimentId={experiment.id} cohorts={(await mock.listCohorts(MOCK_ORG_ID)).items} dirty previewCohort={preview} runOnCohort={vi.fn()} onClose={() => undefined} />);
    expect(screen.getByText(en.run.saveFirst)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: en.run.submit })).toBeDisabled();
  });

  it("promotes only after a run, with a version and notes", async () => {
    const { a } = await setup();
    await userEvent.click(screen.getByRole("button", { name: en.workbench.promote }));
    await userEvent.type(screen.getByLabelText(en.workbench.promoteVersion), "2026.11.9");
    await userEvent.type(screen.getByLabelText(en.workbench.promoteNotes), "lower threshold");
    await userEvent.click(screen.getByRole("button", { name: en.workbench.promoteSubmit }));
    await waitFor(() => expect(a.promote).toHaveBeenCalledWith({ version: "2026.11.9", notes: "lower threshold" }));
    expect(await screen.findByText(en.workbench.promoted("2026.11.9"))).toBeInTheDocument();
  });
});

describe("navigation and mock seeds", () => {
  it("lists Research for the roles that read analysis results", () => {
    expect(navItems("/o", "researcher").some((l) => l.href === "/o/research/experiments")).toBe(true);
    expect(navItems("/o", "viewer").some((l) => l.href === "/o/research/experiments")).toBe(false);
  });

  it("the mock seeds an experiment the workbench opens with a profile the registry can read", async () => {
    const e = await createMockClient().getExperiment(MOCK_RESEARCH_IDS.experimentDraft);
    expect(valueOf(e.profile, spec("metrics.ivtVelocityUnitsPerSec"))).toBe(4);
  });
});
