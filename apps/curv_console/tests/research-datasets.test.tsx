/**
 * WP-X.11: research datasets in the console. The hash display shows the backend's content hash short, full and
 * copyable; the split summary renders the server's per-band counts with a total row; the form sends only what was
 * filled in; the run dialog opens on a dataset ("Start run from this dataset") and starts the run with `dataset_id`.
 */
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { DatasetForm } from "@/components/research/DatasetForm";
import { DatasetHash } from "@/components/research/DatasetHash";
import { RunDialog, type RunStartInput } from "@/components/research/RunDialog";
import { SplitSummary } from "@/components/research/SplitSummary";
import { createMockClient, MOCK_ORG_ID } from "@/lib/api/mock";
import { MOCK_RESEARCH_IDS } from "@/lib/api/mock/research";
import type { CreateDatasetRequest, Dataset } from "@/lib/api/research";
import { shortHash, splitRows, SHORT_HASH_CHARS } from "@/lib/research/datasets";
import { researchMessages } from "@/messages/research";

const push = vi.fn();
const refresh = vi.fn();
vi.mock("next/navigation", () => ({ useRouter: () => ({ push, refresh, back: vi.fn() }) }));
beforeEach(() => {
  push.mockClear();
  refresh.mockClear();
});

const en = researchMessages.en;
const ne = researchMessages.ne;
const HASH = "0123456789abcdef".repeat(4);

describe("hash display", () => {
  it("shortens a long hash and leaves a short one alone", () => {
    expect(shortHash(HASH)).toBe(`${HASH.slice(0, SHORT_HASH_CHARS)}…`);
    expect(shortHash("abc")).toBe("abc");
    expect(shortHash("")).toBe("");
  });

  it("shows the short hash, the full hash and copies the full one", async () => {
    // user-event installs its own clipboard stub; spy on it.
    const user = userEvent.setup();
    const writeText = vi.spyOn(navigator.clipboard, "writeText");
    render(<DatasetHash hash={HASH} />);
    const box = screen.getByTestId("dataset-hash");
    expect(box).toHaveAttribute("data-hash", HASH);
    expect(within(box).getByText(shortHash(HASH))).toHaveAttribute("title", HASH);
    expect(within(box).getByText(HASH)).toBeTruthy();
    await user.click(screen.getByRole("button", { name: en.datasets.hash.copy }));
    expect(writeText).toHaveBeenCalledWith(HASH);
    expect(await screen.findByText(en.datasets.hash.copied)).toBeTruthy();
  });
});

describe("split summary", () => {
  const split: Dataset["split"] = { seed: 42, fractions: { train: 0.7, validation: 0.15, holdout: 0.15 }, strata: "age_band" };
  const counts = [
    { stratum: "24-35", train: 13, validation: 3, holdout: 3 },
    { stratum: "36-47", train: 7, validation: 2, holdout: 1 },
    { stratum: "unbanded", train: 1, validation: 0, holdout: 0 },
  ];

  it("keeps the server's strata and adds a total row", () => {
    const rows = splitRows({ split, split_counts: counts });
    expect(rows.map((r) => [r.stratum, r.train, r.validation, r.holdout, r.all, r.total])).toEqual([
      ["24-35", 13, 3, 3, 19, false],
      ["36-47", 7, 2, 1, 10, false],
      ["unbanded", 1, 0, 0, 1, false],
      ["", 21, 5, 4, 30, true],
    ]);
    expect(splitRows({ split: null, split_counts: [] })).toEqual([]);
    expect(splitRows({ split, split_counts: [] })).toEqual([
      { stratum: "", total: true, train: 0, validation: 0, holdout: 0, all: 0 },
    ]);
  });

  it("renders seed, shares and the table, and says when there is no split", () => {
    const { unmount } = render(<SplitSummary dataset={{ split, split_counts: counts }} text={en} />);
    expect(screen.getByText(en.datasets.splitSeed("42"))).toBeTruthy();
    expect(screen.getByText(en.datasets.splitFractions("0.7", "0.15", "0.15"))).toBeTruthy();
    const table = screen.getByRole("table", { name: en.datasets.splitTitle });
    const total = table.querySelector('[data-split-row="total"]') as HTMLElement;
    expect(within(total).getByText(en.datasets.total)).toBeTruthy();
    expect(within(total).getByText("30")).toBeTruthy();
    expect(within(table.querySelector('[data-split-row="unbanded"]') as HTMLElement).getByText(en.datasets.unbanded)).toBeTruthy();
    unmount();
    render(<SplitSummary dataset={{ split: null, split_counts: [] }} text={ne} />);
    expect(screen.getByText(ne.datasets.noSplit)).toBeTruthy();
  });
});

describe("dataset form", () => {
  it("sends only the filled-in rules, labels and split, then opens the dataset", async () => {
    const user = userEvent.setup();
    const create = vi.fn<(input: CreateDatasetRequest) => Promise<{ ok: true; id: string }>>(async () => ({ ok: true, id: "d-new" }));
    render(<DatasetForm orgId="o" cohorts={[{ id: "c1", name: "All" }]} create={create} />);
    const f = en.datasets.form;
    await user.click(screen.getByRole("button", { name: f.save }));
    expect(screen.getByRole("alert")).toHaveTextContent(f.nameRequired);
    expect(create).not.toHaveBeenCalled();

    await user.selectOptions(screen.getByLabelText(f.cohort), "c1");
    await user.type(screen.getByLabelText(f.name), "Toddlers v1");
    await user.type(screen.getByLabelText(f.minValidRatio), "0.6");
    await user.click(screen.getByLabelText(f.excludePoorTracking));
    await user.type(screen.getByLabelText(f.excludeSessions), "S-03\nS-03\nS-07");
    await user.type(screen.getByLabelText(f.excludeReason), "recorded twice");
    await user.click(screen.getByLabelText(f.labels));
    await user.click(screen.getByLabelText(f.split));
    await user.type(screen.getByLabelText(f.seed), "42");
    await user.click(screen.getByRole("button", { name: f.save }));
    await waitFor(() => expect(create).toHaveBeenCalledTimes(1));
    expect(create.mock.calls[0]![0]).toEqual({
      name: "Toddlers v1",
      cohort_id: "c1",
      exclusion_rules: [
        { kind: "quality_gates", id: "quality", min_valid_ratio: 0.6, exclude_tracking_verdicts: ["poor"] },
        {
          kind: "explicit_list",
          id: "manual",
          sessions: [{ session_id: "S-03", reason: "recorded twice" }, { session_id: "S-07", reason: "recorded twice" }],
        },
      ],
      labels: "review_outcomes",
      split: { seed: 42, fractions: { train: 0.7, validation: 0.15, holdout: 0.15 } },
    });
    expect(push).toHaveBeenCalledWith("/o/research/datasets/d-new");
  });

  it("names a taken name and bad shares; says when there is no cohort", async () => {
    const user = userEvent.setup();
    const results = [
      { ok: false as const, slug: "conflict", status: 409 },
      { ok: false as const, slug: "validation-failed", status: 400, params: [{ name: "split.fractions", reason: "x" }] },
    ];
    const create = vi.fn(async () => results.shift()!);
    const { unmount } = render(<DatasetForm orgId="o" cohorts={[{ id: "c1", name: "All" }]} create={create} />);
    const f = en.datasets.form;
    await user.selectOptions(screen.getByLabelText(f.cohort), "c1");
    await user.type(screen.getByLabelText(f.name), "Taken");
    await user.click(screen.getByRole("button", { name: f.save }));
    expect(await screen.findByText(f.nameTaken)).toBeTruthy();
    await user.click(screen.getByRole("button", { name: f.save }));
    expect(await screen.findByText(f.fractionsInvalid)).toBeTruthy();
    unmount();
    render(<DatasetForm orgId="o" cohorts={[]} create={create} />);
    expect(screen.getByText(f.noCohorts)).toBeTruthy();
  });
});

describe("run dialog on a dataset", () => {
  it("opens on the dataset with its seed and starts the run with dataset_id", async () => {
    const api = createMockClient();
    const dataset = await api.getDataset(MOCK_RESEARCH_IDS.datasetToddlers);
    const cohorts = (await api.listCohorts(MOCK_ORG_ID)).items;
    const runOnCohort = vi.fn<(input: RunStartInput) => Promise<{ ok: true; runId: string }>>(async () => ({ ok: true, runId: "r-new" }));
    const user = userEvent.setup();
    render(
      <RunDialog
        orgId="o"
        experimentId={MOCK_RESEARCH_IDS.experimentDraft}
        cohorts={cohorts}
        datasets={[dataset]}
        initialDatasetId={dataset.id}
        dirty={false}
        previewCohort={vi.fn()}
        runOnCohort={runOnCohort}
        onClose={vi.fn()}
      />,
    );
    expect(screen.getByLabelText(en.run.sourceDataset)).toBeChecked();
    expect(screen.getByLabelText(en.run.dataset)).toHaveValue(dataset.id);
    expect(screen.getByLabelText(en.run.seed)).toHaveValue("42");
    expect(screen.getByText(en.run.datasetCounts(dataset.member_count, shortHash(dataset.content_hash)))).toBeTruthy();
    expect(screen.queryByRole("button", { name: en.run.preview })).toBeNull();
    await user.click(screen.getByRole("button", { name: en.run.submit }));
    await waitFor(() => expect(runOnCohort).toHaveBeenCalledTimes(1));
    const input = runOnCohort.mock.calls[0]![0];
    expect(input.dataset_id).toBe(dataset.id);
    expect(input.cohort_id).toBeUndefined();
    expect(input.seed).toBe(42);
    expect(push).toHaveBeenCalledWith("/o/research/runs/r-new");

    // Switching back to a cohort needs a cohort before it can start.
    await user.click(screen.getByLabelText(en.run.sourceCohort));
    expect(screen.getByRole("button", { name: en.run.submit })).toBeDisabled();
  });
});
