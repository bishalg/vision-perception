import { expect, test } from "@playwright/test";
import { LIVE, loginToOverview } from "./helpers";

test.skip(LIVE, "mock-mode flow: it relies on the seeded mock audit log");

test("audit log: filter by action prefix and clear the filters", async ({ page }) => {
  const org = await loginToOverview(page);
  await page.goto(`/${org}/audit`);
  await expect(page.getByRole("cell", { name: "review.update" }).first()).toBeVisible();
  await page.getByLabel("Action (prefix)").fill("org.");
  await page.getByRole("button", { name: "Apply filters" }).click();
  await expect(page).toHaveURL(/action=org\./);
  await expect(page.getByRole("cell", { name: "org.update" }).first()).toBeVisible();
  await expect(page.getByRole("cell", { name: "review.update" })).toHaveCount(0);
  await page.getByLabel("Action (prefix)").fill("nothing.matches.");
  await page.getByRole("button", { name: "Apply filters" }).click();
  await expect(page.getByText("No audit entries match these filters.")).toBeVisible();
  await page.getByRole("link", { name: "Clear filters" }).click();
  await expect(page.getByRole("cell", { name: "review.update" }).first()).toBeVisible();
});
