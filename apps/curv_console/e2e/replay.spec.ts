/**
 * WP-X.20 session replay and workbench tuning view in mock mode (ADR-0015 V-1..V-4, V-6). The mock session records
 * carry deterministic synthetic frames (two clips, 60 Hz, one blink, one loss gap); every event, ribbon, trace and
 * band is computed in the browser by the Rust core (asd-wasm).
 */
import { type Page } from "@playwright/test";
import { expect, test, LIVE, login, loginToOverview, paths, OVERVIEW_URL } from "./helpers";

test.skip(LIVE, "mock-mode flows: they rely on the synthetic mock frames");

async function openReplay(page: Page): Promise<void> {
  const org = await loginToOverview(page);
  await page.goto(paths(org).clinic.sessions);
  const rows = page.locator("table.data tbody tr");
  await rows.first().getByRole("link").click();
  await expect(page).toHaveURL(new RegExp(`${paths(org).clinic.sessions}/[^/?]+$`));
  await page.getByRole("navigation", { name: "Session sections" }).getByRole("link", { name: "Replay" }).click();
  await expect(page).toHaveURL(/tab=replay/);
}

test("replay tab: scrub, ribbon segments, trace band, table, CSV, clip switch, quality playhead", async ({ page }) => {
  await openReplay(page);
  const canvas = page.getByTestId("replay-canvas");
  await expect(canvas).toHaveAttribute("role", "img");
  await expect(canvas).toHaveAttribute("aria-label", /^Gaze replay of Social versus geometric split screen at 0\.00 s/);
  await expect(page.getByTestId("replay-clip")).toHaveCount(2, { timeout: 30_000 });

  // Ribbon segments and the expected band come from the core.
  await expect.poll(() => page.locator('[data-testid="replay-ribbon"] rect').count(), { timeout: 30_000 }).toBeGreaterThan(3);
  await expect(page.locator('[data-testid="replay-ribbon"] rect[data-aoi="Actor_Face"]').first()).toBeAttached();
  await expect(page.locator('[data-testid="replay-bands"] rect')).toHaveCount(1, { timeout: 30_000 });
  await expect(page.getByTestId("replay-trace")).toContainText("Expected 0.62");
  await expect(page.getByTestId("replay-trace")).toContainText("Atypical below 0.35");

  // Scrub: the canvas label follows the scrubber; the keyboard steps one frame.
  await page.getByRole("slider", { name: "Clip time" }).fill("4.5");
  await expect(canvas).toHaveAttribute("aria-label", /at 4\.50 s/);
  await canvas.focus();
  await page.keyboard.press("ArrowRight");
  await expect(canvas).toHaveAttribute("data-time", /^4\.51[67]$/);
  await page.keyboard.press("Space");
  await expect(page.getByRole("button", { name: "Pause" })).toBeVisible();
  await page.keyboard.press("Space");
  await expect(page.getByRole("button", { name: "Play" })).toBeVisible();

  // Table toggle and CSV download (header checked).
  await page.getByRole("button", { name: "Table" }).click();
  await expect(page.getByTestId("replay-trace-table").locator("tbody tr").first()).toBeVisible();
  const [download] = await Promise.all([page.waitForEvent("download"), page.getByRole("button", { name: "Download CSV" }).click()]);
  expect(download.suggestedFilename()).toBe("replay-trace-stim_01_social_geo_10s.csv");
  const path = await download.path();
  const { readFileSync } = await import("node:fs");
  expect(readFileSync(path, "utf8").split("\r\n")[0]).toBe("clip_id,metric,t_sec,value,valid_ratio,n_frames");
  await page.getByRole("button", { name: "Chart" }).click();

  // Switch clip: the canvas, the trace and the bands follow (two baseline segments).
  await page.getByRole("button", { name: "Show Gaze cue to a toy" }).click();
  await expect(canvas).toHaveAttribute("data-clip", "stim_02_gaze_cue_8s");
  await expect(page.locator('[data-testid="replay-bands"] rect')).toHaveCount(2, { timeout: 30_000 });
  await expect(page.getByText("No cohort heatmap: this session is not linked to an analysis run.")).toBeVisible();

  // The Quality tab reads the same playhead (V-4).
  await page.getByRole("slider", { name: "Clip time" }).fill("2");
  await page.getByRole("navigation", { name: "Session sections" }).getByRole("link", { name: "Quality" }).click();
  await expect(page.getByTestId("quality-playhead")).toHaveText("Replay position: Gaze cue to a toy at 2.00 s");
});

test("workbench tuning toggle renders the published and the draft replay side by side", async ({ page }) => {
  await login(page, "researcher@example.org");
  await expect(page).toHaveURL(OVERVIEW_URL);
  const org = new URL(page.url()).pathname.split("/")[1]!;
  await page.goto(paths(org).research.experiments);
  await page.getByRole("link", { name: "New experiment" }).click();
  await page.getByLabel("Name", { exact: true }).fill(`E2E replay ${Date.now()}`);
  await page.getByRole("button", { name: "Create experiment" }).click();
  await expect(page.locator('[data-param-id="metrics.fixationAlgorithm"]')).toBeVisible({ timeout: 45_000 });

  await page.getByRole("button", { name: "Replay", exact: true }).click();
  const tuning = page.getByTestId("tuning-replay");
  await expect(tuning.getByTestId("replay-canvas")).toHaveCount(2, { timeout: 30_000 });
  await expect(tuning.getByRole("heading", { name: "Published parameters" })).toBeVisible();
  await expect(tuning.getByRole("heading", { name: "Draft parameters" })).toBeVisible();
  await expect(tuning.getByTestId("replay-timeline")).toHaveCount(2);

  // A replay-only knob changes the draft side (re-run through the core, debounced).
  await page.locator('[data-param-id="metrics.fixationAlgorithm"] select').selectOption("idt");
  await expect(tuning.getByRole("alert")).toHaveCount(0);
  await page.getByRole("button", { name: "Replay", exact: true }).click();
  await expect(page.getByTestId("tuning-replay")).toHaveCount(0);
});
