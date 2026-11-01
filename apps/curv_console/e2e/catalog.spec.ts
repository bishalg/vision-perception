/**
 * WP-C.4 catalog editor (mock mode). Global config documents are changed by superadmins only, so these flows sign in
 * as the mock superadmin and open the demo org. The mock gives every login the same user id: a draft created here
 * cannot be approved by the same user (four-eyes), while the seeded draft, authored by someone else, can.
 */
import { type Page } from "@playwright/test";
import { expect, test, LIVE, MOCK_ORG_ID, login, loginToOverview } from "./helpers";

test.skip(LIVE, "mock-mode flows: they rely on the seeded mock catalog versions");

const nav = (page: Page) => page.getByRole("navigation", { name: "Main navigation" });

async function superadminToCatalog(page: Page): Promise<void> {
  await login(page, "superadmin@example.org");
  await expect(page.getByRole("heading", { name: "Choose an organisation" })).toBeVisible();
  await page.getByLabel("Organisation id").fill(MOCK_ORG_ID);
  await page.getByRole("button", { name: "Switch" }).click();
  await expect(page).toHaveURL(new RegExp(`/${MOCK_ORG_ID}/overview$`));
  await nav(page).getByRole("link", { name: "Catalog" }).click();
  await expect(page).toHaveURL(new RegExp(`/${MOCK_ORG_ID}/config/catalog$`));
  await expect(page.getByRole("heading", { name: "Stimulus catalog" })).toBeVisible();
}

test("catalog: a non-superadmin sees the version history read-only", async ({ page }) => {
  const org = await loginToOverview(page);
  await nav(page).getByRole("link", { name: "Catalog" }).click();
  await expect(page).toHaveURL(new RegExp(`/${org}/config/catalog$`));
  await expect(page.getByText("Published now: 2026.10.1")).toBeVisible();
  await expect(page.getByRole("link", { name: /New draft/ })).toHaveCount(0);
  await page.getByRole("link", { name: "2026.10.1" }).click();
  await expect(page.getByRole("heading", { name: /2026\.10\.1/ })).toBeVisible();
  await expect(page.getByRole("button", { name: "Approve" })).toHaveCount(0);
  await expect(page.getByRole("region", { name: "express_30s" })).toBeVisible();
});

test("catalog editor: compose a draft with a new AOI, see server errors per field, then four-eyes on approve", async ({ page }) => {
  await superadminToCatalog(page);
  await page.getByRole("link", { name: "New draft from 2026.10.1" }).click();
  await expect(page.getByRole("heading", { name: "New catalog draft" })).toBeVisible();

  // Second clip: add an AOI at the playhead and give it the mouth role.
  const clips = page.getByRole("navigation", { name: "Clip list" });
  await clips.getByRole("button", { name: /Joint Attention & Gaze Following/ }).click();
  await page.getByLabel("Playhead (s)").fill("2");
  await page.getByRole("button", { name: "Add AOI at playhead" }).click();
  await page.getByLabel(/^AOI name/).fill("E2E_Mouth");
  await page.getByRole("checkbox", { name: "Mouth" }).check();
  await expect(page.getByRole("button", { name: "Frame with AOI boxes: E2E_Mouth" })).toBeVisible();
  await expect(page.getByRole("button", { name: /^E2E_Mouth 2–8 s$/ })).toBeVisible();

  // Batteries: move the first Express clip down.
  const express = page.getByRole("region", { name: "express_30s" });
  await express.getByRole("button", { name: "Move down: stim_05_geopref_split_10s" }).click();

  // An invalid version passes the shape check and comes back from the server as a field error.
  await page.getByLabel(/^New version/).fill("bad version");
  await page.getByRole("button", { name: "Save as draft" }).click();
  await expect(page.getByRole("alert").filter({ hasText: "The server rejected some fields." })).toContainText("version: must be 1 to 64");
  await expect(page.getByLabel(/^New version/)).toHaveAttribute("aria-invalid", "true");

  const version = `e2e-${Date.now()}`;
  await page.getByLabel(/^New version/).fill(version);
  await page.getByLabel(/^Notes/).fill("E2E draft");
  await page.getByRole("button", { name: "Save as draft" }).click();
  await expect(page).toHaveURL(new RegExp(`/config/catalog/${version}\\?created=1$`));
  await expect(page.getByText("Draft saved.")).toBeVisible();

  // The saved body carries the edits.
  await page.getByRole("navigation", { name: "Clip list" }).getByRole("button", { name: /Joint Attention & Gaze Following/ }).click();
  await expect(page.getByRole("cell", { name: "E2E_Mouth" })).toBeVisible();
  const expressView = page.getByRole("region", { name: "express_30s" });
  await expect(expressView.locator("li").first()).toContainText("stim_01_social_peekaboo_8s");

  // The author cannot approve their own draft.
  await page.getByRole("button", { name: "Approve" }).click();
  await expect(page.getByRole("status").filter({ hasText: "Four-eyes rule" })).toBeVisible();
});

test("catalog lifecycle: approve and publish someone else's draft, then compare with the old version", async ({ page }) => {
  await superadminToCatalog(page);
  await page.getByRole("link", { name: "2026.11.0-rc2" }).click();
  await expect(page.getByRole("heading", { name: /2026\.11\.0-rc2/ })).toBeVisible();
  await page.getByRole("button", { name: "Approve" }).click();
  await expect(page.getByText("Approved. Another superadmin can now publish it.")).toBeVisible();
  await page.getByRole("button", { name: "Publish" }).click();
  await expect(page.getByText("Published. The previous published version was retired.")).toBeVisible();
  await expect(page.getByRole("heading", { name: /2026\.11\.0-rc2/ })).toContainText("Published");

  await page.getByRole("combobox", { name: "Compare with" }).selectOption("2026.10.1");
  await page.getByRole("button", { name: "Show changes" }).click();
  const diff = page.getByRole("region", { name: "Changes from 2026.10.1 to 2026.11.0-rc2" });
  await expect(diff).toContainText("stim_11_name_call_orient_6s");
  await expect(diff).toContainText("stim_09_social_turn_taking_10s: areas");

  await page.getByRole("link", { name: "All catalog versions" }).click();
  await expect(page.getByText("Published now: 2026.11.0-rc2")).toBeVisible();
});
