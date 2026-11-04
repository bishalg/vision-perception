import { expect, test, LIVE, login, loginToOverview } from "./helpers";

test.skip(LIVE, "mock-mode flow: it relies on the seeded mock audit log");

/** WP-L.1a: a mock user whose locale is `ne` (the `+ne` email tag) sees Devanagari headings under `lang="ne-NP"`. */
async function loginNe(page: import("@playwright/test").Page): Promise<string> {
  await login(page, "admin+ne@example.org");
  await expect(page).toHaveURL(/\/[0-9a-f-]{36}\/overview$/);
  await expect(page.locator("html")).toHaveAttribute("lang", "ne-NP");
  return new URL(page.url()).pathname.split("/")[1]!;
}


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

test("audit log renders in Nepali", async ({ page }) => {
  const org = await loginNe(page);
  await page.goto(`/${org}/audit`);
  await expect(page.getByRole("heading", { level: 1 })).toHaveText("अडिट लग");
  await expect(page.getByRole("button", { name: "फिल्टर लागू गर्नुहोस्" })).toBeVisible();
});
