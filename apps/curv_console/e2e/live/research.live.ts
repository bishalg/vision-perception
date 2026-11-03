/**
 * Live research e2e (WP-X.7 against WP-X.2 cohorts and WP-X.3 runs). Not run in the mock suite: it needs a backend with
 * the researcher toolkit routes (they answer `501 not-implemented` before X.2 / X.3). Set E2E_RESEARCH=1 plus the
 * variables of `support.ts` (E2E_PASSWORD, E2E_ORG, E2E_RESEARCHER_EMAIL); run it with the live config:
 * `E2E_BASE_URL=... npx playwright test -c e2e/live/playwright.live.config.ts research`.
 *
 * Flow: a researcher saves a cohort (counts-only preview), starts a run from an experiment, watches the status leave
 * queued/running, and reads the run's provenance and results. It never asserts a count: below the small-cell threshold
 * the page shows "n < k", and an organisation's data decides which it is.
 */
import { expect, test } from "../helpers";
import { signIn } from "./support";

const RESEARCH_LIVE = Boolean(process.env.E2E_RESEARCH);
test.skip(!RESEARCH_LIVE, "set E2E_RESEARCH=1 with a backend that serves cohorts and analysis runs (WP-X.2, WP-X.3)");

const org = () => process.env.E2E_ORG ?? "";
const email = () => process.env.E2E_RESEARCHER_EMAIL ?? "researcher@example.org";

test("a researcher saves a cohort, runs an experiment on it and reads the finished run", async ({ page }) => {
  await signIn(page, email(), org());
  await page.goto(`/${org()}/research/cohorts/new`);
  await page.getByLabel("Name", { exact: true }).fill(`Live cohort ${Date.now()}`);
  await page.getByRole("button", { name: "Preview counts" }).click();
  await expect(page.getByTestId("cohort-preview")).toBeVisible();
  await page.getByRole("button", { name: "Save cohort" }).click();
  await expect(page).toHaveURL(/\/research\/cohorts\/[^/]+$/);

  await page.goto(`/${org()}/research/experiments`);
  const experiment = page.getByRole("table", { name: "Experiments" }).getByRole("link").first();
  await experiment.click();
  await page.getByRole("button", { name: "Run on cohort" }).click();
  const dialog = page.getByRole("dialog", { name: "Run on cohort" });
  await dialog.getByRole("combobox", { name: /^Cohort/ }).selectOption({ index: 1 });
  await dialog.getByRole("button", { name: "Start run" }).click();
  await expect(page).toHaveURL(/\/research\/runs\/[^/]+$/, { timeout: 60_000 });

  // The status leaves queued and running by polling alone; any final state ends the poll.
  await expect(page.getByTestId("run-status")).toHaveText(/Finished|Failed|Cancelled/, { timeout: 240_000 });
  await expect(page.getByRole("region", { name: "Provenance" })).toContainText("Definition hash");
});
