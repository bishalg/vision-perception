import { expect, test, LIVE, login, loginToOverview, paths, OVERVIEW_URL, navTo } from "./helpers";

test.skip(LIVE, "mock-mode flows: they rely on the seeded mock exports and destinations");


test("exports: request an export and open its detail page", async ({ page }) => {
  const org = await loginToOverview(page);
  await navTo(page, "Data", "Exports");
  await expect(page).toHaveURL(new RegExp(`${paths(org).data.exports}$`));
  await page.getByLabel(/^Reason/).fill("E2E cohort extract for the study");
  await page.getByRole("button", { name: "Request export" }).click();
  await expect(page.getByText("Export queued.")).toBeVisible();
  await page.getByRole("link", { name: "View export" }).click();
  await expect(page).toHaveURL(new RegExp(`${paths(org).data.exports}/[0-9a-f-]{36}$`));
  await expect(page.getByText("Queued").first()).toBeVisible();
});

test("integrations: add a webhook, set a generated secret once and run the test", async ({ page }) => {
  const org = await loginToOverview(page);
  await navTo(page, "Administration", "Integrations");
  await expect(page).toHaveURL(new RegExp(`${paths(org).admin.integrations}$`));
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
  await navTo(page, "Administration", "Integrations");
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
  await expect(page).toHaveURL(new RegExp(`${paths(org).admin.integrations}$`));
  await expect(page.getByRole("link", { name: "E2E delete me" })).toHaveCount(0);
});

test("exports and integrations render in Nepali for a user whose locale is ne", async ({ page }) => {
  await login(page, "admin+ne@example.org");
  await expect(page).toHaveURL(OVERVIEW_URL);
  const org = new URL(page.url()).pathname.split("/")[1]!;
  await page.goto(paths(org).data.exports);
  await expect(page.locator("html")).toHaveAttribute("lang", "ne-NP");
  await expect(page.getByRole("heading", { name: "निर्यात", level: 1 })).toBeVisible();
  await expect(page.getByRole("button", { name: "निर्यात अनुरोध गर्नुहोस्" })).toBeVisible();
  await page.goto(paths(org).admin.integrations);
  await expect(page.getByRole("heading", { name: "एकीकरण", level: 1 })).toBeVisible();
});
