/**
 * WP-X.7: cohorts and runs pages. `useJobPoll` stops on every final state and on unmount; the matrix and histograms
 * show "n < k" for suppressed cells (never 0) and shade the published baseline range; the provenance block lists every
 * RUN-5 field; the run status panel follows the mock progression; the export form pre-fills from a run.
 */
import { act, render, renderHook, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { CohortPreview } from "@/components/research/CohortPreview";
import { DeltaBars } from "@/components/research/DeltaBars";
import { Histogram } from "@/components/research/Histogram";
import { ProvenanceBlock, PROVENANCE_FIELDS } from "@/components/research/ProvenanceBlock";
import { RunResults } from "@/components/research/RunResults";
import { RunSessionsTable } from "@/components/research/RunSessionsTable";
import { RunStatus } from "@/components/research/RunStatus";
import { TransitionMatrix } from "@/components/research/TransitionMatrix";
import { ExportForm } from "@/components/exports/ExportForm";
import { useJobPoll } from "@/hooks/useJobPoll";
import { createMockClient, MOCK_ORG_ID } from "@/lib/api/mock";
import { MOCK_RESEARCH_IDS } from "@/lib/api/mock/research";
import { baselineRange, driftThresholds } from "@/lib/research/baselines";
import { describeCohort } from "@/lib/research/cohort";
import { snapshotOf, type RunSnapshot } from "@/lib/research/runs";
import { researchMessages } from "@/messages/research";

const refresh = vi.fn();
const push = vi.fn();
vi.mock("next/navigation", () => ({ useRouter: () => ({ push, refresh, back: vi.fn() }) }));

const en = researchMessages.en;
const K = 12;

beforeEach(() => {
  refresh.mockClear();
  push.mockClear();
});
afterEach(() => vi.useRealTimers());

/** A baselines body shaped like `NormativeBaselineSet` of config-bundle-v1.json. */
function bundleBaselines(ranges: Record<string, { acceptableLow?: number | null; acceptableHigh?: number | null; expected?: number | null }>[]) {
  return {
    schemaVersion: "1.0.0",
    status: "published",
    ageBrackets: [{ id: "24-35", minMonths: 24, maxMonths: 35 }],
    videos: Object.fromEntries(
      ranges.map((byAge, i) => [
        `clip_${i}`,
        {
          title: "t", category: "c", weight: 1,
          segments: [{ id: "s", description: "d", startSec: 0, endSec: 5, expectations: [{ biomarker: "eye_dwell_ratio", direction: "higher", citation: "c", source: "literature", byAge }] }],
        },
      ]),
    ),
  };
}

describe("baselineRange and driftThresholds", () => {
  it("reads the range of a metric for an age band, falling back to the default key", () => {
    const body = bundleBaselines([{ "24-35": { acceptableLow: 0.25, acceptableHigh: 0.5, expected: 0.38 }, default: { acceptableLow: 0.1, acceptableHigh: 0.9 } }]);
    expect(baselineRange(body, "eye_dwell_ratio", "24-35")).toEqual({ low: 0.25, high: 0.5, expected: 0.38 });
    expect(baselineRange(body, "eye_dwell_ratio", "36-47")).toEqual({ low: 0.1, high: 0.9, expected: null });
  });

  it("shades nothing when the shape is unknown, the metric is absent or occurrences disagree", () => {
    expect(baselineRange({}, "eye_dwell_ratio", "24-35")).toBeNull();
    expect(baselineRange(null, "eye_dwell_ratio", "24-35")).toBeNull();
    expect(baselineRange("x", "eye_dwell_ratio", "24-35")).toBeNull();
    const body = bundleBaselines([{ "24-35": { acceptableLow: 0.25, acceptableHigh: 0.5 } }]);
    expect(baselineRange(body, "mouth_dwell_ratio", "24-35")).toBeNull();
    const open = bundleBaselines([{ "24-35": { acceptableLow: null, acceptableHigh: 0.5 } }]);
    expect(baselineRange(open, "eye_dwell_ratio", "24-35")).toBeNull();
    const clash = bundleBaselines([{ "24-35": { acceptableLow: 0.25, acceptableHigh: 0.5 } }, { "24-35": { acceptableLow: 0.3, acceptableHigh: 0.5 } }]);
    expect(baselineRange(clash, "eye_dwell_ratio", "24-35")).toBeNull();
    const same = bundleBaselines([{ "24-35": { acceptableLow: 0.25, acceptableHigh: 0.5 } }, { "24-35": { acceptableLow: 0.25, acceptableHigh: 0.5 } }]);
    expect(baselineRange(same, "eye_dwell_ratio", "24-35")).toEqual({ low: 0.25, high: 0.5, expected: null });
  });

  it("reads the drift thresholds of the scoring profile and never invents one", () => {
    expect(driftThresholds({ trackingQuality: { drift: { warnDeg: 1.5, hardDeg: 3 } } })).toEqual({ warnDeg: 1.5, hardDeg: 3 });
    expect(driftThresholds({ trackingQuality: { drift: { warnDeg: 1.5 } } })).toEqual({ warnDeg: 1.5, hardDeg: null });
    expect(driftThresholds({ flag: {} })).toEqual({ warnDeg: null, hardDeg: null });
    expect(driftThresholds(undefined)).toEqual({ warnDeg: null, hardDeg: null });
    expect(driftThresholds({ trackingQuality: { drift: { warnDeg: "1.5" } } })).toEqual({ warnDeg: null, hardDeg: null });
  });
});

describe("useJobPoll", () => {
  type Job = { status: string };
  const run = (fetch: () => Promise<Job>, extra: { enabled?: boolean } = {}) =>
    renderHook((p: { enabled: boolean }) => useJobPoll<Job>({ enabled: p.enabled, jobKey: "j", intervalMs: 1000, fetch, isFinal: (j) => j.status !== "running" }), {
      initialProps: { enabled: extra.enabled ?? true },
    });

  for (const final of ["succeeded", "failed", "cancelled"]) {
    it(`stops polling on ${final}`, async () => {
      vi.useFakeTimers();
      const answers = ["running", "running", final];
      const fetch = vi.fn(async () => ({ status: answers.shift() ?? final }));
      const { result } = run(fetch);
      await act(async () => { await vi.advanceTimersByTimeAsync(3000); });
      expect(fetch).toHaveBeenCalledTimes(3);
      expect(result.current.state).toBe("final");
      expect(result.current.value).toEqual({ status: final });
      await act(async () => { await vi.advanceTimersByTimeAsync(10_000); });
      expect(fetch).toHaveBeenCalledTimes(3);
    });
  }

  it("stops on unmount and when disabled", async () => {
    vi.useFakeTimers();
    const fetch = vi.fn(async () => ({ status: "running" }));
    const first = run(fetch);
    await act(async () => { await vi.advanceTimersByTimeAsync(2000); });
    expect(fetch).toHaveBeenCalledTimes(2);
    first.unmount();
    await act(async () => { await vi.advanceTimersByTimeAsync(10_000); });
    expect(fetch).toHaveBeenCalledTimes(2);

    fetch.mockClear();
    const second = run(fetch);
    second.rerender({ enabled: false });
    await act(async () => { await vi.advanceTimersByTimeAsync(5000); });
    expect(fetch).not.toHaveBeenCalled();
    expect(second.result.current.state).toBe("idle");
  });

  it("stops on a failing poll and reports the error", async () => {
    vi.useFakeTimers();
    const boom = new Error("boom");
    const fetch = vi.fn(async () => { throw boom; });
    const { result } = run(fetch);
    await act(async () => { await vi.advanceTimersByTimeAsync(5000); });
    expect(fetch).toHaveBeenCalledTimes(1);
    expect(result.current.state).toBe("error");
    expect(result.current.error).toBe(boom);
  });

  it("stops after maxPolls", async () => {
    vi.useFakeTimers();
    const fetch = vi.fn(async () => ({ status: "running" }));
    const { result } = renderHook(() => useJobPoll<Job>({ enabled: true, jobKey: "j", intervalMs: 10, maxPolls: 3, fetch, isFinal: () => false }));
    await act(async () => { await vi.advanceTimersByTimeAsync(1000); });
    expect(fetch).toHaveBeenCalledTimes(3);
    expect(result.current.state).toBe("timeout");
  });
});

describe("TransitionMatrix", () => {
  it("renders n < k for a suppressed cell and never 0", () => {
    render(<TransitionMatrix caption="Changes" k={K} transitions={[["green", "green", 17], ["yellow", "green", null], ["yellow", "yellow", 12]]} />);
    const cells = screen.getAllByTestId("transition-cell").map((c) => c.textContent);
    expect(cells).toContain("n < 12");
    expect(cells).toContain("17");
    expect(cells).not.toContain("0");
    expect(cells.filter((c) => c === "n < 12")).toHaveLength(1);
    // A pair the run did not list is a dash, not a count.
    expect(cells).toContain(en.charts.noTransition);
  });
});

describe("Histogram and DeltaBars", () => {
  const dist = { n: 30, suppressed: false, mean: 0.4, hist: { edges: [0, 0.25, 0.5, 0.75, 1], counts: [3, 12, 10, 5] } };

  it("shades the baseline range behind the bars", () => {
    const { rerender } = render(<Histogram label="Dwell" dist={dist} k={K} baseline={{ low: 0.25, high: 0.5, expected: 0.38 }} />);
    const band = screen.getByTestId("baseline-range");
    expect(Number(band.getAttribute("width"))).toBeGreaterThan(0);
    expect(screen.getAllByTestId("hist-bar")).toHaveLength(4);
    expect(screen.getByText(en.charts.baseline("0.25", "0.50"))).toBeTruthy();
    rerender(<Histogram label="Dwell" dist={dist} k={K} baseline={null} />);
    expect(screen.queryByTestId("baseline-range")).toBeNull();
    expect(screen.getByText(en.charts.noBaseline)).toBeTruthy();
  });

  it("shows n < k and no bars for a suppressed distribution", () => {
    render(<Histogram label="Dwell" dist={{ n: null, suppressed: true, hist: null }} k={K} baseline={{ low: 0, high: 1, expected: null }} />);
    expect(screen.getByText("n < 12")).toBeTruthy();
    expect(screen.queryAllByTestId("hist-bar")).toHaveLength(0);
    expect(screen.queryByTestId("baseline-range")).toBeNull();
  });

  it("DeltaBars draws one bar per shown band and lists a suppressed band as n < k", () => {
    render(<DeltaBars label="Composite" k={K} rows={[{ label: "24-35", delta: { n: 30, mean: 0.03, ci95_mean: [0.01, 0.05] } }, { label: "36-47", delta: { n: null, suppressed: true, mean: null } }]} />);
    expect(screen.getAllByTestId("delta-bar")).toHaveLength(1);
    expect(screen.getByTestId("delta-suppressed").textContent).toBe("36-47: n < 12");
  });
});

describe("mock runs and results", () => {
  const client = () => createMockClient();

  it("a new run reaches running and then succeeded by polling; the seeded running run stays running", async () => {
    const api = client();
    const created = await api.createAnalysisRun(MOCK_ORG_ID, { kind: "cohort_rescore", experiment_id: MOCK_RESEARCH_IDS.experimentDraft, cohort_id: MOCK_RESEARCH_IDS.cohortToddlers });
    expect(created.status).toBe("queued");
    const seen: string[] = [];
    for (let i = 0; i < 6; i++) seen.push((await api.getAnalysisRun(created.id)).status);
    expect(seen).toEqual(["queued", "running", "running", "running", "succeeded", "succeeded"]);
    const done = await api.getAnalysisRun(created.id);
    expect(done.summary?.counts.sessions).toBe(37);
    expect((await api.listAnalysisRunSessions(created.id)).items.length).toBeGreaterThan(0);
    for (let i = 0; i < 10; i++) expect((await api.getAnalysisRun(MOCK_RESEARCH_IDS.runRunning)).status).toBe("running");
    expect((await api.getAnalysisRun(MOCK_RESEARCH_IDS.runFailed)).error).toBe("budget_exceeded");
  });

  it("a running run can be cancelled and a finished one cannot", async () => {
    const api = client();
    expect((await api.cancelAnalysisRun(MOCK_RESEARCH_IDS.runRunning)).status).toBe("cancelled");
    await expect(api.cancelAnalysisRun(MOCK_RESEARCH_IDS.runSucceeded)).rejects.toMatchObject({ status: 409 });
  });

  it("the effective config carries drift thresholds and a baseline set the helpers can read", async () => {
    const cfg = await client().getEffectiveConfig(MOCK_ORG_ID);
    expect(driftThresholds(cfg.scoring_profile.body)).toEqual({ warnDeg: 1.5, hardDeg: 3 });
    expect(baselineRange(cfg.normative_baselines.body, "eye_dwell_ratio", "24-35")).toMatchObject({ low: 0.25, high: 0.5 });
  });

  it("the run results show hidden cells as n < k and shade the metric's baseline range", async () => {
    const api = client();
    const run = await api.getAnalysisRun(MOCK_RESEARCH_IDS.runSucceeded);
    const cfg = await api.getEffectiveConfig(MOCK_ORG_ID);
    render(<RunResults summary={run.summary!} baselines={cfg.normative_baselines.body} />);
    const suppressedFlags = screen.getAllByTestId("flag-count").filter((c) => c.textContent === "n < 12");
    expect(suppressedFlags.length).toBe(4);
    expect(screen.getAllByTestId("flag-count").map((c) => c.textContent)).not.toContain("0");
    const small = document.querySelector('[data-band="36-47"]')!;
    expect(within(small as HTMLElement).getByText("n < 12")).toBeTruthy();
    expect(screen.getAllByTestId("baseline-range").length).toBeGreaterThan(0);
    expect(screen.getAllByTestId("transition-cell").map((c) => c.textContent)).toContain("n < 12");
  });

  it("per-session rows never show a token, MRN or device model", async () => {
    const api = client();
    const rows = (await api.listAnalysisRunSessions(MOCK_RESEARCH_IDS.runSucceeded, { limit: 5 })).items;
    render(<RunSessionsTable rows={rows} />);
    expect(screen.getAllByRole("row")).toHaveLength(6);
    expect(Object.keys(rows[0]!)).not.toContain("participant_token");
  });
});

describe("ProvenanceBlock", () => {
  it("lists every RUN-5 field with a copy button", async () => {
    const run = await createMockClient().getAnalysisRun(MOCK_RESEARCH_IDS.runSucceeded);
    render(<ProvenanceBlock run={run} />);
    const run5 = [
      "definition_hash", "cohort_hash", "membership_hash", "profile_hash", "published_profile_version", "baselines_version",
      "catalog_version", "bundle_version", "core_version", "contract_version", "pipeline_version", "registry_version", "seed", "level_requested",
    ];
    for (const f of run5) expect(PROVENANCE_FIELDS).toContain(f);
    for (const f of PROVENANCE_FIELDS) {
      const row = document.querySelector(`[data-provenance="${f}"]`) as HTMLElement;
      expect(row, f).toBeTruthy();
      expect(within(row).getByText(en.runs.provenance.fields[f]!)).toBeTruthy();
    }
    // A hash is shown in full and copyable; a field the backend left empty says so.
    expect(screen.getByText("f".repeat(64))).toBeTruthy();
    expect(within(document.querySelector('[data-provenance="bundle_version"]') as HTMLElement).getByText(en.runs.provenance.notRecorded)).toBeTruthy();
    expect(screen.getAllByRole("button", { name: /^Copy / }).length).toBeGreaterThanOrEqual(12);
  });
});

describe("RunStatus", () => {
  const snap = (status: RunSnapshot["status"], over: Partial<RunSnapshot> = {}): RunSnapshot => ({ status, progress: null, error: null, ...over });
  const noop = async () => ({ ok: true as const, runId: "new" });

  function setup(initial: RunSnapshot, poll: (id: string) => Promise<{ ok: true; run: RunSnapshot }> = async () => ({ ok: true, run: initial }), extra: Partial<{ canRun: boolean; canExport: boolean }> = {}) {
    const cancel = vi.fn(async () => ({ ok: true as const, run: snap("cancelled") }));
    const rerun = vi.fn(noop);
    render(<RunStatus orgId="org" runId="run1" initial={initial} poll={poll} cancel={cancel} rerun={rerun} canRun={extra.canRun ?? true} canExport={extra.canExport ?? true} pollMs={5} />);
    return { cancel, rerun };
  }

  it("offers cancel only while queued or running", () => {
    for (const status of ["queued", "running"] as const) {
      const { unmount } = render(<RunStatus orgId="org" runId="r" initial={snap(status)} poll={async () => ({ ok: true, run: snap(status) })} cancel={async () => ({ ok: true, run: snap("cancelled") })} rerun={noop} canRun canExport pollMs={5} />);
      expect(screen.getByRole("button", { name: en.runs.cancel })).toBeTruthy();
      unmount();
    }
    for (const status of ["succeeded", "failed", "cancelled"] as const) {
      const { unmount } = render(<RunStatus orgId="org" runId="r" initial={snap(status)} poll={async () => ({ ok: true, run: snap(status) })} cancel={async () => ({ ok: true, run: snap("cancelled") })} rerun={noop} canRun canExport pollMs={5} />);
      expect(screen.queryByRole("button", { name: en.runs.cancel })).toBeNull();
      expect(screen.getByRole("button", { name: en.runs.rerun })).toBeTruthy();
      unmount();
    }
  });

  it("hides cancel and rerun from a role that cannot run analyses", () => {
    setup(snap("running"), undefined, { canRun: false });
    expect(screen.queryByRole("button", { name: en.runs.cancel })).toBeNull();
  });

  it("follows a run from running to succeeded, refreshes once and offers the export pre-fill", async () => {
    const answers: RunSnapshot[] = [snap("running", { progress: { done: 10, total: 37, phase: "evaluate" } }), snap("succeeded")];
    const poll = vi.fn(async () => ({ ok: true as const, run: answers.shift() ?? snap("succeeded") }));
    setup(snap("queued"), poll);
    await waitFor(() => expect(screen.getByTestId("run-status").textContent).toBe(en.runStatus.running));
    await waitFor(() => expect(screen.getByTestId("run-status").textContent).toBe(en.runStatus.succeeded));
    expect(refresh).toHaveBeenCalledTimes(1);
    const calls = poll.mock.calls.length;
    await new Promise((r) => setTimeout(r, 40));
    expect(poll.mock.calls.length).toBe(calls);
    const link = screen.getByRole("link", { name: en.runs.exportResults });
    expect(link.getAttribute("href")).toBe("/org/exports?format=analysis_results&run=run1");
    expect(screen.queryByRole("button", { name: en.runs.cancel })).toBeNull();
  });

  it("shows a final state the page re-render brought before the poll saw it", async () => {
    const poll = async () => ({ ok: true as const, run: snap("running") });
    const props = { orgId: "org", runId: "run1", poll, cancel: async () => ({ ok: true as const, run: snap("cancelled") }), rerun: noop, canRun: true, canExport: true, pollMs: 5 };
    const view = render(<RunStatus {...props} initial={snap("queued")} />);
    await waitFor(() => expect(screen.getByTestId("run-status").textContent).toBe(en.runStatus.running));
    view.rerender(<RunStatus {...props} initial={snap("succeeded")} />);
    expect(screen.getByTestId("run-status").textContent).toBe(en.runStatus.succeeded);
    expect(screen.queryByRole("button", { name: en.runs.cancel })).toBeNull();
  });

  it("shows the budget_exceeded copy and its reason code for a failed run", () => {
    setup(snap("failed", { error: "budget_exceeded" }));
    expect(screen.getByRole("alert").textContent).toContain(en.runs.failure.budget_exceeded);
    expect(screen.getByText(en.runs.failure.code("budget_exceeded"))).toBeTruthy();
  });

  it("cancels a running run", async () => {
    const { cancel } = setup(snap("running"));
    await userEvent.click(screen.getByRole("button", { name: en.runs.cancel }));
    await waitFor(() => expect(screen.getByTestId("run-status").textContent).toBe(en.runStatus.cancelled));
    expect(cancel).toHaveBeenCalledWith("run1");
    expect(screen.queryByRole("button", { name: en.runs.cancel })).toBeNull();
  });

  it("keeps polling after the backend accepts a cancel with the run still running (202, live)", async () => {
    let cancelled = false;
    const poll = vi.fn(async () => ({ ok: true as const, run: snap(cancelled ? "cancelled" : "running") }));
    const cancel = vi.fn(async () => {
      cancelled = true;
      return { ok: true as const, run: snap("running") };
    });
    render(<RunStatus orgId="org" runId="run1" initial={snap("running")} poll={poll} cancel={cancel} rerun={noop} canRun canExport pollMs={5} />);
    await userEvent.click(screen.getByRole("button", { name: en.runs.cancel }));
    await waitFor(() => expect(screen.getByTestId("run-status").textContent).toBe(en.runStatus.cancelled));
    expect(cancel).toHaveBeenCalledWith("run1");
  });

  it("runs again and goes to the new run", async () => {
    const { rerun } = setup(snap("failed", { error: "budget_exceeded" }));
    await userEvent.click(screen.getByRole("button", { name: en.runs.rerun }));
    await waitFor(() => expect(push).toHaveBeenCalledWith("/org/research/runs/new"));
    expect(rerun).toHaveBeenCalledWith("run1");
  });
});

describe("cohorts", () => {
  it("CohortPreview shows n < k for suppressed counts, never 0", () => {
    render(
      <CohortPreview preview={{ n: null, by_age_band: [{ band: "36-47", n: null, frames_n: null, suppressed: true }, { band: "24-35", n: 30, frames_n: 10, suppressed: false }], by_level: { frames: 12, score: 0 }, excluded: { no_consent: 1, withdrawn: 0, unscorable: 0, no_age: 0, quality: 0 }, small_cell_k: K }} />,
    );
    const view = screen.getByTestId("cohort-preview");
    expect(view.textContent).toContain("36-47: n < 12");
    expect(view.textContent).toContain("24-35: 30");
    expect(view.textContent).toContain("n < 12 sessions");
    expect(view.textContent).not.toMatch(/(^|\s)0 sessions/);
  });

  it("describes a definition and an empty one", () => {
    expect(describeCohort({})).toEqual([en.cohorts.summary.all]);
    expect(describeCohort({ age_bands: ["24-35"], require_frames: true })).toEqual([en.cohorts.summary.ageBands("24-35"), en.cohorts.summary.framesOnly]);
  });
});

describe("export pre-fill", () => {
  const form = (create = vi.fn(async () => ({ ok: true as const, id: "x1" }))) => {
    render(<ExportForm create={create} canCreate canIdentified destinations={[]} orgId="org" prefill={{ runId: "run-77" }} />);
    return create;
  };

  it("fixes the format and mode, shows the run and limits the purpose", () => {
    form();
    expect(screen.getByTestId("prefill-run").textContent).toBe("run-77");
    expect(screen.queryByLabelText("Format")).toBeNull();
    expect(screen.queryByLabelText("Mode")).toBeNull();
    const purpose = screen.getByLabelText("Purpose") as HTMLSelectElement;
    expect([...purpose.options].map((o) => o.value)).toEqual(["research", "quality_improvement"]);
    const hidden = (name: string) => (document.querySelector(`input[name="${name}"]`) as HTMLInputElement).value;
    expect(hidden("format")).toBe("analysis_results");
    expect(hidden("mode")).toBe("deidentified");
    expect(hidden("analysis_run_id")).toBe("run-77");
  });

  it("offers Parquet for per-session rows only and submits the run", async () => {
    const create = form();
    const encoding = () => [...(screen.getByLabelText("File type") as HTMLSelectElement).options].map((o) => o.value);
    expect(encoding()).toEqual(["jsonl", "csv"]);
    await userEvent.selectOptions(screen.getByLabelText("Table"), "sessions");
    expect(encoding()).toEqual(["jsonl", "csv", "parquet"]);
    await userEvent.type(screen.getByLabelText(/^Reason/), "Compare the draft profile");
    await userEvent.click(screen.getByRole("button", { name: "Request export" }));
    await waitFor(() => expect(create).toHaveBeenCalled());
    const fd = (create.mock.calls[0] as unknown as [FormData])[0];
    expect(fd.get("analysis_run_id")).toBe("run-77");
    expect(fd.get("analysis_layout")).toBe("sessions");
    expect(fd.get("analysis_encoding")).toBe("jsonl");
    expect(fd.get("purpose")).toBe("research");
  });

  it("the mock accepts an analysis_results export request", async () => {
    const api = createMockClient();
    const created = await api.createExport(MOCK_ORG_ID, {
      format: "analysis_results", mode: "deidentified", purpose: "research", reason: "Compare the draft profile", filters: {},
      analysis: { run_id: MOCK_RESEARCH_IDS.runSucceeded, layout: "aggregates", encoding: "csv" },
    });
    expect(created.format).toBe("analysis_results");
  });
});

describe("run snapshot", () => {
  it("keeps only what changes while a run executes", async () => {
    const run = await createMockClient().getAnalysisRun(MOCK_RESEARCH_IDS.runRunning);
    expect(snapshotOf(run)).toEqual({ status: "running", progress: { done: 120, total: 300, phase: "evaluate" }, error: null });
  });
});
