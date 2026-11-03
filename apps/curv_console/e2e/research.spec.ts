/**
 * WP-X.5 research area in mock mode (RUI-1..5, RUI-7; acceptance 3, 7 and the console half of 8): the registry form,
 * the workbench with its live preview and the run dialog. The preview re-scores in the browser through the Rust core
 * (asd-wasm, copied into public/ by `scripts/prepare-asd-wasm.mjs`); the run page itself is WP-X.7.
 */
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { type Page } from "@playwright/test";
import { expect, test, LIVE, login } from "./helpers";

test.skip(LIVE, "mock-mode flows: they rely on the seeded mock research data");

const nav = (page: Page) => page.getByRole("navigation", { name: "Main navigation" });
const registry = JSON.parse(readFileSync(resolve(process.cwd(), "src/lib/research/param-registry-v1.json"), "utf8")) as {
  params: { id: string }[];
};

async function researcher(page: Page): Promise<void> {
  await login(page, "researcher@example.org");
  await expect(page).toHaveURL(/\/[0-9a-f-]{36}\/overview$/);
}

async function newExperiment(page: Page, name: string): Promise<void> {
  await nav(page).getByRole("link", { name: "Research", exact: true }).click();
  await expect(page.getByRole("heading", { name: "Research", exact: true })).toBeVisible();
  await expect(page.getByText("This is not a diagnostic tool.")).toBeVisible();
  await page.getByRole("link", { name: "All experiments" }).click();
  await page.getByRole("link", { name: "New experiment" }).click();
  await page.getByLabel("Name", { exact: true }).fill(name);
  await page.getByRole("button", { name: "Create experiment" }).click();
  await expect(page.getByRole("heading", { name })).toBeVisible({ timeout: 45_000 });
}

test("a viewer has no Research entry; an admin reads the experiments list but cannot create one", async ({ page }) => {
  await login(page, "viewer@example.org");
  await expect(page).toHaveURL(/\/overview$/);
  await expect(nav(page).getByRole("link", { name: "Research", exact: true })).toHaveCount(0);
  const org = new URL(page.url()).pathname.split("/")[1]!;
  await page.goto(`/${org}/research`);
  await expect(page.getByText("Your role cannot view research experiments.")).toBeVisible();

  await page.context().clearCookies();
  await login(page);
  await nav(page).getByRole("link", { name: "Research", exact: true }).click();
  await page.getByRole("link", { name: "All experiments" }).click();
  await expect(page.getByRole("table", { name: "Experiments" })).toContainText("Lower I-VT threshold");
  await expect(page.getByRole("link", { name: "New experiment" })).toHaveCount(0);
});

test("the workbench renders one control per registry row with the reprocess badges (acceptance 3)", async ({ page }) => {
  await researcher(page);
  await newExperiment(page, `E2E controls ${Date.now()}`);
  await expect(page.locator("[data-param-id]")).toHaveCount(registry.params.length);
  await expect(page.locator('[data-param-id="flag.greenFloor"]')).toContainText("Green floor");
  await expect(page.locator('[data-param-id="flag.greenFloor"]')).toContainText("score");
  // capture_only knobs are disabled in the preview.
  await expect(page.locator('[data-param-id="trackingQuality.calibration.maxRetriesPerTarget"] input[type="number"]')).toBeDisabled();

  const green = page.getByLabel("Green floor", { exact: true });
  await green.fill("1.5");
  await expect(green).toHaveAttribute("aria-invalid", "true");
  await expect(page.getByText("flag.greenFloor must be in [0, 1]").first()).toBeVisible();
  await expect(page.getByRole("button", { name: "Save experiment" })).toBeDisabled();
});

test("new experiment: knob moves both cards, save, run on a cohort (acceptance 7)", async ({ page }) => {
  await researcher(page);
  const name = `E2E workbench ${Date.now()}`;
  await newExperiment(page, name);

  const cards = page.getByRole("article");
  await expect(cards).toHaveCount(2, { timeout: 30_000 });
  await expect(page.getByText("The Rust core could not be loaded.")).toHaveCount(0);
  const draftValue = (i: number) => cards.nth(i).getByLabel("Composite").nth(1);
  const publishedValue = (i: number) => cards.nth(i).getByLabel("Composite").nth(0);
  await expect(publishedValue(0)).toHaveText(/\d/);
  const before = [await publishedValue(0).innerText(), await publishedValue(1).innerText()];

  // A knob the score level applies: the draft side recomputes, the published column does not move.
  const share = page.getByLabel("Low-confidence share", { exact: true });
  const diff = page.getByText(/parameters? differs? from the published profile/);
  const diffBefore = await diff.innerText();
  await share.fill("0.05");
  await expect(diff).not.toHaveText(diffBefore);
  await expect(page.getByRole("button", { name: "Save experiment" })).toBeEnabled();
  expect([await publishedValue(0).innerText(), await publishedValue(1).innerText()]).toEqual(before);
  await expect(draftValue(0)).toHaveText(/\d/);

  // A frames-level knob: not applied at the score level, said so on the card.
  await page.getByLabel("Fixation velocity threshold", { exact: true }).fill("4");
  await expect(cards.nth(0)).toContainText("param_not_applied: metrics.ivtVelocityUnitsPerSec");

  await page.getByRole("button", { name: "Save experiment" }).click();
  await expect(page.getByText("Saved.")).toBeVisible();
  await expect(page.getByRole("button", { name: "Save experiment" })).toBeDisabled();

  await page.getByRole("button", { name: "Run on cohort" }).click();
  const dialog = page.getByRole("dialog", { name: "Run on cohort" });
  await dialog.getByRole("combobox", { name: /^Cohort/ }).selectOption({ index: 1 });
  await dialog.getByRole("button", { name: "Preview counts" }).click();
  await expect(dialog.getByRole("status")).toContainText("sessions");
  await dialog.getByRole("button", { name: "Start run" }).click();
  await expect(page).toHaveURL(/\/research\/runs\/[^/]+$/, { timeout: 45_000 });
});

test("single-session sweep draws the curve and creates no job (acceptance 8)", async ({ page }) => {
  await researcher(page);
  await newExperiment(page, `E2E sweep ${Date.now()}`);
  await expect(page.getByRole("article")).toHaveCount(2, { timeout: 30_000 });
  await page.getByRole("combobox", { name: /^Parameter/ }).selectOption("flag.greenFloor");
  await page.getByLabel("From", { exact: true }).fill("0.5");
  await page.getByLabel("To", { exact: true }).fill("0.8");
  await page.getByLabel("Step", { exact: true }).fill("0.05");
  await page.getByRole("button", { name: "Run sweep" }).click();
  await expect(page.getByTestId("sweep-curve")).toBeVisible();
  await expect(page.getByRole("table", { name: "Composite score against Green floor" }).getByRole("row")).toHaveCount(8);
});

/** WP-X.7: the cohorts and runs pages (RUI-5, RUI-6), driven by the mock progression (`mock/research.ts`). */
async function openRuns(page: Page): Promise<void> {
  await researcher(page);
  await nav(page).getByRole("link", { name: "Research", exact: true }).click();
  await page.getByRole("navigation", { name: "Research" }).getByRole("link", { name: "Analysis runs" }).click();
  await expect(page.getByRole("table", { name: "Analysis runs" })).toBeVisible();
}

test("cohorts: list, create with a counts-only preview, and the saved cohort page", async ({ page }) => {
  await researcher(page);
  await nav(page).getByRole("link", { name: "Research", exact: true }).click();
  await page.getByRole("navigation", { name: "Research" }).getByRole("link", { name: "Cohorts" }).click();
  await expect(page.getByRole("table", { name: "Cohorts" })).toContainText("All consented sessions");
  await page.getByRole("link", { name: "New cohort" }).click();
  await page.getByLabel("Age bands", { exact: true }).fill("36-47");
  await page.getByRole("button", { name: "Preview counts" }).click();
  // 7 sessions are below the small-cell threshold (12): the count is hidden, never shown as a number.
  await expect(page.getByTestId("cohort-preview")).toContainText("36-47: n < 12");
  await page.getByLabel("Name", { exact: true }).fill(`E2E cohort ${Date.now()}`);
  await page.getByRole("button", { name: "Save cohort" }).click();
  await expect(page).toHaveURL(/\/research\/cohorts\/[^/]+$/, { timeout: 45_000 });
  await expect(page.getByTestId("cohort-preview")).toContainText("n < 12");
});

test("a failed run shows the budget_exceeded copy and no cancel button", async ({ page }) => {
  await openRuns(page);
  await page.getByRole("combobox", { name: "Status" }).selectOption("failed");
  await page.getByRole("button", { name: "Apply filter" }).click();
  await page.getByRole("link", { name: "Failed" }).first().click();
  await expect(page.getByRole("alert").filter({ hasText: "The run did not finish." })).toContainText("over the compute budget");
  await expect(page.getByText("Reason code: budget_exceeded")).toBeVisible();
  await expect(page.getByRole("button", { name: "Cancel run" })).toHaveCount(0);
  await expect(page.getByRole("button", { name: "Run again" })).toBeVisible();
});

test("a running run offers cancel and no re-run", async ({ page }) => {
  await openRuns(page);
  await page.getByRole("link", { name: "Running" }).first().click();
  await expect(page.getByTestId("run-status")).toHaveText("Running");
  await expect(page.getByRole("button", { name: "Cancel run" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Run again" })).toHaveCount(0);
  await expect(page.getByRole("progressbar")).toBeVisible();
});

test("run again goes queued -> running -> finished, then shows provenance, results and the export pre-fill", async ({ page }) => {
  await openRuns(page);
  await page.getByRole("combobox", { name: "Status" }).selectOption("failed");
  await page.getByRole("button", { name: "Apply filter" }).click();
  await page.getByRole("link", { name: "Failed" }).first().click();
  const failedUrl = page.url();
  // The button is in the server HTML before React hydrates it: a click that lands too early does nothing, so retry.
  await expect(async () => {
    await page.getByRole("button", { name: "Run again" }).click();
    await expect(page).not.toHaveURL(failedUrl, { timeout: 4_000 });
  }).toPass({ timeout: 60_000 });
  await expect(page).toHaveURL(/\/research\/runs\/[^/]+$/);
  await expect(page.getByTestId("run-status")).toHaveText(/Queued|Running/);
  await expect(page.getByTestId("run-status")).toHaveText("Running", { timeout: 30_000 });
  // Cancel is offered while it runs and gone once it finished.
  await expect(page.getByRole("button", { name: "Cancel run" })).toBeVisible();
  await expect(page.getByTestId("run-status")).toHaveText("Finished", { timeout: 30_000 });
  await expect(page.getByRole("button", { name: "Cancel run" })).toHaveCount(0);

  // The page reloads with the results; RUN-5 provenance is complete and suppressed cells say "n < 12".
  await expect(page.getByRole("region", { name: "Provenance" })).toContainText("Definition hash");
  for (const label of ["Cohort hash", "Profile hash", "Baselines version", "Core version", "Registry version", "Seed"]) {
    await expect(page.getByRole("region", { name: "Provenance" }).getByText(label, { exact: true })).toBeVisible();
  }
  await expect(page.getByRole("region", { name: "Age band 36-47" })).toContainText("n < 12");
  await expect(page.getByTestId("baseline-range").first()).toBeVisible();

  await page.getByRole("link", { name: "Per-session results" }).click();
  await expect(page.getByRole("table", { name: "Per-session results" })).toContainText("SESS-MOCK-0001");
  await page.getByRole("link", { name: "Back to the run" }).click();

  await page.getByRole("link", { name: "Export results…" }).click();
  await expect(page).toHaveURL(/\/exports\?format=analysis_results&run=/);
  await expect(page.getByTestId("prefill-run")).toBeVisible();
  await expect(page.getByLabel("Format")).toHaveCount(0);
  await expect(page.getByLabel("Purpose").locator("option")).toHaveText(["Research", "Quality improvement"]);
  await page.getByLabel(/^Reason/).fill("Compare the draft profile with the published one");
  await page.getByRole("button", { name: "Request export" }).click();
  await expect(page.getByText("Export queued.")).toBeVisible({ timeout: 45_000 });
});

test("the Quality tab draws the drift warn and hard lines from the effective profile", async ({ page }) => {
  await researcher(page);
  const org = new URL(page.url()).pathname.split("/")[1]!;
  await page.goto(`/${org}/sessions?tracking=poor`);
  const rows = page.locator("table.data tbody tr");
  test.skip((await rows.count()) === 0, "no poor-tracking sessions in this org");
  await rows.first().getByRole("link").click();
  await page.getByRole("navigation", { name: "Session sections" }).getByRole("link", { name: "Quality" }).click();
  await expect(page.getByRole("img", { name: /Gaze offset in degrees/ })).toBeVisible();
  await expect(page.getByTestId("drift-warn")).toContainText("Warn 1.5°");
  await expect(page.getByTestId("drift-hard")).toContainText("Hard limit 3°");
});

/** WP-X.9: the sweep and age-profile sections of the seeded phase-B runs (SWP-3, AGE-1, AGE-3; `mock/research.ts`). */
const SWEEP_RUN = "a0000000-0000-4000-8000-000000000004";
const AGE_RUN = "a0000000-0000-4000-8000-000000000005";

test("a two-axis sweep run draws all 625 grid cells in each heatmap and lists the off-grid published value", async ({ page }) => {
  await researcher(page);
  const org = new URL(page.url()).pathname.split("/")[1]!;
  await page.goto(`/${org}/research/runs/${SWEEP_RUN}`);
  const panel = page.getByRole("region", { name: "Parameter sweep" });
  await expect(panel).toContainText("626 cells");
  await expect(panel.getByTestId("sweep-heatmap")).toHaveCount(2);
  await expect(panel.getByTestId("sweep-cell")).toHaveCount(1250);
  await expect(panel).toContainText("The published value 0.65, 0.35 is outside the grid");
  await panel.getByText("Show every cell as a table").click();
  const table = panel.getByRole("table", { name: "Sweep cells" });
  await expect(table.getByRole("row")).toHaveCount(627);
  await expect(table.locator("tr[data-published]")).toContainText("(published)");
  await expect(table).toContainText("n < 12");
});

test("an age-profile run shows the shares under both baselines and hides the small band", async ({ page }) => {
  await researcher(page);
  const org = new URL(page.url()).pathname.split("/")[1]!;
  await page.goto(`/${org}/research/runs/${AGE_RUN}`);
  const section = page.getByRole("region", { name: "Age profile" });
  await expect(section).toContainText("Published baselines 2026.10.0; experiment baselines 2026.11.0-draft.");
  const shown = section.getByRole("region", { name: "Age band 24-35 (24-35 months)" });
  await expect(shown.getByTestId("age-row")).toContainText("50% on target, 30% monitor, 20% atypical");
  await expect(shown.getByTestId("age-row")).toContainText("30% on target, 50% monitor, 20% atypical");
  await expect(section.getByRole("region", { name: "Age band 36-47 (36-47 months)" }).getByTestId("age-band-suppressed")).toHaveText("n < 12");
});
