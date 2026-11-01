import { expect, test, LIVE, loginToOverview } from "./helpers";

test.skip(LIVE, "mock-mode flows: they rely on the seeded mock exports and destinations");

const nav = (page: import("@playwright/test").Page) => page.getByRole("navigation", { name: "Main navigation" });

test("exports: request an export and open its detail page", async ({ page }) => {
  const org = await loginToOverview(page);
  await nav(page).getByRole("link", { name: "Exports" }).click();
  await expect(page).toHaveURL(new RegExp(`/${org}/exports$`));
  await page.getByLabel(/^Reason/).fill("E2E cohort extract for the study");
  await page.getByRole("button", { name: "Request export" }).click();
  await expect(page.getByText("Export queued.")).toBeVisible();
  await page.getByRole("link", { name: "View export" }).click();
  await expect(page).toHaveURL(new RegExp(`/${org}/exports/[0-9a-f-]{36}$`));
  await expect(page.getByText("Queued").first()).toBeVisible();
});

test("integrations: add a webhook, set a generated secret once and run the test", async ({ page }) => {
  const org = await loginToOverview(page);
  await nav(page).getByRole("link", { name: "Integrations" }).click();
  await expect(page).toHaveURL(new RegExp(`/${org}/settings/integrations$`));
  await page.getByLabel("Name").fill("E2E hook");
  await page.getByLabel("Receiver URL (https only)").fill("https://hooks.example.org/e2e");
  await page.getByRole("button", { name: "Add destination" }).click();
  await page.getByRole("link", { name: "Open it to set up its credential." }).click();
  await page.getByRole("button", { name: "Set secret" }).click();
  await expect(page.getByTestId("generated-secret")).toHaveText(/^whsec_/);
  await page.getByRole("button", { name: "Send test" }).click();
  await expect(page.getByText("The test delivery succeeded.")).toBeVisible();
});

test("integrations: delete a destination after an in-page confirmation", async ({ page }) => {
  const org = await loginToOverview(page);
  await nav(page).getByRole("link", { name: "Integrations" }).click();
  await page.getByLabel("Name").fill("E2E delete me");
  await page.getByLabel("Receiver URL (https only)").fill("https://hooks.example.org/e2e-delete");
  await page.getByRole("button", { name: "Add destination" }).click();
  await page.getByRole("link", { name: "Open it to set up its credential." }).click();
  await page.getByRole("button", { name: "Delete destination" }).click();
  await expect(page.getByText(/Delete "E2E delete me"\?/)).toBeVisible();
  await page.getByRole("button", { name: "Cancel" }).click();
  await expect(page.getByText(/Delete "E2E delete me"\?/)).toHaveCount(0);
  await page.getByRole("button", { name: "Delete destination" }).click();
  await page.getByRole("button", { name: "Yes, delete" }).click();
  await expect(page).toHaveURL(new RegExp(`/${org}/settings/integrations$`));
  await expect(page.getByRole("link", { name: "E2E delete me" })).toHaveCount(0);
});
