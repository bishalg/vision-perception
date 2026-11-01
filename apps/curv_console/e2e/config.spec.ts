/**
 * WP-C.5 baselines and scoring-profile editors, sandbox re-score and bundles (mock mode). Global config documents are
 * changed by superadmins only, so the lifecycle flows sign in as the mock superadmin and open the demo org. The
 * sandbox and the editors' validation run the Rust core (asd-wasm) in the browser: the dev server is started after
 * `scripts/prepare-asd-wasm.mjs` copied the build into public/.
 */
import { expect, test, type Page } from "@playwright/test";
import { LIVE, MOCK_ORG_ID, login, loginToOverview } from "./helpers";

test.skip(LIVE, "mock-mode flows: they rely on the seeded mock config versions");

const nav = (page: Page) => page.getByRole("navigation", { name: "Main navigation" });

async function superadminTo(page: Page, link: string, heading: string): Promise<void> {
  await login(page, "superadmin@example.org");
  await expect(page.getByRole("heading", { name: "Choose an organisation" })).toBeVisible();
  await page.getByLabel("Organisation id").fill(MOCK_ORG_ID);
  await page.getByRole("button", { name: "Switch" }).click();
  await expect(page).toHaveURL(new RegExp(`/${MOCK_ORG_ID}/overview$`));
  await nav(page).getByRole("link", { name: link, exact: true }).click();
  await expect(page.getByRole("heading", { name: heading, exact: true })).toBeVisible();
}

test("config pages: an admin reads the history read-only and sees the bundle history", async ({ page }) => {
  await loginToOverview(page);
  await nav(page).getByRole("link", { name: "Baselines", exact: true }).click();
  await expect(page.getByText("Published now: 2026.10.1")).toBeVisible();
  await expect(page.getByRole("link", { name: /New draft/ })).toHaveCount(0);
  await page.getByRole("link", { name: "2026.10.1" }).click();
  await expect(page.getByRole("button", { name: "Approve" })).toHaveCount(0);
  await expect(page.getByRole("region", { name: "stim_01_social_geo_10s" })).toBeVisible();

  await nav(page).getByRole("link", { name: "Bundles", exact: true }).click();
  await expect(page.getByRole("heading", { name: "Config bundles" })).toBeVisible();
  await expect(page.getByRole("table", { name: "Config bundles" })).toContainText("2026.10.10-1");
});

test("scoring profile editor: the Rust core flags a range problem at its field, then the draft saves", async ({ page }) => {
  await superadminTo(page, "Scoring", "Scoring profile");
  await page.getByRole("link", { name: "New draft from 2026.10.1" }).click();
  await expect(page.getByRole("heading", { name: "New scoring profile draft" })).toBeVisible();
  await expect(page.getByText("The Rust core accepts this document.")).toBeVisible();

  const green = page.getByLabel("Green floor (composite)");
  await green.fill("1.5");
  await expect(green).toHaveAttribute("aria-invalid", "true");
  await expect(page.getByText("flag.greenFloor must be in [0, 1]").first()).toBeVisible();
  await expect(page.getByText(/Not applied by the Rust core scorer yet: it uses a fixed 0\.6/)).toBeVisible();

  await green.fill("0.7");
  await expect(page.getByText("The Rust core accepts this document.")).toBeVisible();
  const version = `e2e-score-${Date.now()}`;
  await page.getByLabel(/^New version/).fill(version);
  await page.getByRole("button", { name: "Save as draft" }).click();
  await expect(page.getByText("A changelog note is required.")).toBeVisible();
  await page.getByLabel(/^Changelog note/).fill("E2E stricter green floor");
  await page.getByRole("button", { name: "Save as draft" }).click();
  await expect(page).toHaveURL(new RegExp(`/config/scoring/${version}\\?created=1$`));
  await expect(page.getByText(/Draft saved\./)).toBeVisible();
  await expect(page.getByRole("region", { name: "Flag thresholds" })).toContainText("0.7");

  // Four-eyes: the author cannot approve their own draft.
  await page.getByRole("button", { name: "Approve" }).click();
  await expect(page.getByRole("status").filter({ hasText: "Four-eyes rule" })).toBeVisible();
});

test("sandbox gate: the approved profile publishes only after a sandbox run of it", async ({ page }) => {
  await superadminTo(page, "Scoring", "Scoring profile");
  await page.getByRole("link", { name: "2026.11.0-rc1" }).click();
  await expect(page.getByRole("button", { name: "Publish" })).toBeDisabled();
  await expect(page.getByText("No sandbox run of this version in this browser yet.")).toBeVisible();

  await page.getByRole("link", { name: "Run the sandbox with this version" }).click();
  await expect(page.getByRole("heading", { name: "Sandbox re-score" })).toBeVisible();
  await expect(page.getByText(/lowConfidenceValidRatio is not applied/)).toBeVisible();
  await expect(page.getByText(/"unreliable" flag is not reproduced/)).toBeVisible();
  // The re-score reads the stored record, so the old API-form caveat about one session-wide valid ratio is gone.
  await expect(page.getByText(/stored record with its own per-clip valid-sample ratios/)).toBeVisible();
  await expect(page.getByText(/^Rust core /)).toBeVisible();
  await page.getByRole("button", { name: "Run re-score" }).click();
  const summary = page.getByRole("region", { name: "Summary" });
  await expect(summary).toBeVisible();
  await expect(summary.getByTestId("flags-changed")).not.toHaveText("0");
  await expect(page.getByRole("table", { name: "Sandbox re-score" }).getByRole("row")).toHaveCount(11);

  await page.goBack();
  await page.reload();
  await expect(page.getByText(/Sandbox run .*10 sessions/)).toBeVisible();
  await page.getByRole("button", { name: "Publish" }).click();
  await expect(page.getByText("Published. The previous published version was retired.")).toBeVisible();
});

test("baselines: approve the seeded draft, compare with the published set, then compose a bundle", async ({ page }) => {
  await superadminTo(page, "Baselines", "Normative baselines");
  await page.getByRole("link", { name: "2026.11.0-rc1" }).click();
  await page.getByRole("button", { name: "Approve" }).click();
  await expect(page.getByText("Approved. Another superadmin can now publish it.")).toBeVisible();
  await page.getByRole("combobox", { name: "Compare with" }).selectOption("2026.10.1");
  await page.getByRole("button", { name: "Show changes" }).click();
  const diff = page.getByRole("region", { name: "Changes from 2026.10.1 to 2026.11.0-rc1" });
  await expect(diff).toContainText("stim_01_social_geo_10s");
  await expect(diff).toContainText("segments.0.expectations.1.source");

  await nav(page).getByRole("link", { name: "Bundles", exact: true }).click();
  await expect(page.getByRole("region", { name: "Composing now would sign" })).toContainText("2026.10.1");
  await page.getByLabel(/^Feature flags/).fill("sandbox_v2=true");
  await page.getByRole("button", { name: "Compose and sign bundle" }).click();
  await expect(page.getByText(/^Bundle \d{4}\.\d{2}\.\d{2}-\d+ composed and signed\.$/)).toBeVisible();
});

test("baselines editor: the Rust core names a clip without segments", async ({ page }) => {
  await superadminTo(page, "Baselines", "Normative baselines");
  await page.getByRole("link", { name: "New draft from 2026.10.1" }).click();
  await expect(page.getByText("The Rust core accepts this document.")).toBeVisible();
  await page.getByLabel("New clip id").fill("stim_99_new_clip_6s");
  await page.getByRole("button", { name: "Add clip" }).click();
  await expect(page.getByText("stim_99_new_clip_6s: no segments").first()).toBeVisible();
  await expect(
    page.getByRole("navigation", { name: "Clip list" }).getByRole("button", { name: /stim_99_new_clip_6s/ }),
  ).toContainText("has problems");
});
