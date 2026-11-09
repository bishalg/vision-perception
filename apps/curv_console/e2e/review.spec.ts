import { expect, test, LIVE, login, loginToOverview, paths, OVERVIEW_URL, navTo } from "./helpers";

/** WP-L.1a: a mock user whose locale is `ne` (the `+ne` email tag) sees Devanagari headings under `lang="ne-NP"`. */
async function loginNe(page: import("@playwright/test").Page): Promise<string> {
  await login(page, "admin+ne@example.org");
  await expect(page).toHaveURL(OVERVIEW_URL);
  await expect(page.locator("html")).toHaveAttribute("lang", "ne-NP");
  return new URL(page.url()).pathname.split("/")[1]!;
}


test("review queue: keyboard triage closes the selected item (j, then 1 = Confirm)", async ({ page }) => {
  const org = await loginToOverview(page);
  await navTo(page, "Clinic", "Review");
  await expect(page).toHaveURL(new RegExp(`${paths(org).clinic.review}$`));
  await expect(page.getByRole("heading", { name: "Review queue" })).toBeVisible();
  const rows = page.locator("table.review-table tbody tr");
  const before = await rows.count();
  test.skip(before < 2, "needs at least two open review items");

  await expect(rows.nth(0)).toHaveAttribute("aria-current", "true");
  await page.keyboard.press("j");
  await expect(rows.nth(1)).toHaveAttribute("aria-current", "true");
  const who = (await rows.nth(1).locator("td").first().innerText()).trim();
  // Participants can have several sessions; the session link identifies the item.
  const href = (await rows.nth(1).getByRole("link").getAttribute("href"))!;
  const itemLinks = page.locator(`table.review-table a[href="${href}"]`);
  const linksBefore = await itemLinks.count();

  await page.keyboard.press("1");
  await expect(page.getByRole("status")).toHaveText(`Confirm: ${who}`);
  await expect(rows).toHaveCount(before - 1);

  // The decision is persisted: it is gone from the open queue after a reload and listed under Done.
  await page.reload();
  await expect(itemLinks).toHaveCount(linksBefore - 1);
  await page.goto(`${paths(org).clinic.review}?status=done`);
  await expect(page.locator(`table.review-table a[href="${href}"]`).first()).toBeVisible();
});

test("review queue: Flag capture issue asks for a label first", async ({ page }) => {
  const org = await loginToOverview(page);
  await page.goto(paths(org).clinic.review);
  const rows = page.locator("table.review-table tbody tr");
  const before = await rows.count();
  test.skip(before === 0, "needs an open review item");
  await page.keyboard.press("2");
  const picker = page.getByLabel("Capture issue");
  await expect(picker).toBeFocused();
  await picker.selectOption("calibration_drift");
  await page.getByRole("button", { name: "Apply label" }).click();
  await expect(page.getByRole("status")).toContainText("Flag capture issue:");
  await expect(rows).toHaveCount(before - 1);
});

test("review queue renders in Nepali", async ({ page }) => {
  test.skip(LIVE, "locale flows use the mock backend's user locale seed");
  const org = await loginNe(page);
  await page.goto(paths(org).clinic.review);
  await expect(page.getByRole("heading", { level: 1 })).toHaveText("समीक्षा पङ्क्ति");
  await expect(page.getByLabel("अवस्था").first()).toBeVisible();
}); 