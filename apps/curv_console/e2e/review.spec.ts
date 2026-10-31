import { expect, test } from "@playwright/test";
import { loginToOverview } from "./helpers";

test("review queue: keyboard triage closes the selected item (j, then 1 = Confirm)", async ({ page }) => {
  const org = await loginToOverview(page);
  await page.getByRole("navigation", { name: "Main navigation" }).getByRole("link", { name: "Review" }).click();
  await expect(page).toHaveURL(new RegExp(`/${org}/review$`));
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
  await page.goto(`/${org}/review?status=done`);
  await expect(page.locator(`table.review-table a[href="${href}"]`).first()).toBeVisible();
});

test("review queue: Flag capture issue asks for a label first", async ({ page }) => {
  const org = await loginToOverview(page);
  await page.goto(`/${org}/review`);
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
