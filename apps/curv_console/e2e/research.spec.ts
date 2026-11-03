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
