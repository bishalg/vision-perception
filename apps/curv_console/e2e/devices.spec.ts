import { expect, test, LIVE, login, loginToOverview, paths, OVERVIEW_URL, navTo } from "./helpers";

/** WP-L.1a: a mock user whose locale is `ne` (the `+ne` email tag) sees Devanagari headings under `lang="ne-NP"`. */
async function loginNe(page: import("@playwright/test").Page): Promise<string> {
  await login(page, "admin+ne@example.org");
  await expect(page).toHaveURL(OVERVIEW_URL);
  await expect(page.locator("html")).toHaveAttribute("lang", "ne-NP");
  return new URL(page.url()).pathname.split("/")[1]!;
}


test("devices: generate an enrol code shown as text and QR", async ({ page }) => {
  const org = await loginToOverview(page);
  await navTo(page, "Administration", "Devices");
  await expect(page).toHaveURL(new RegExp(`${paths(org).admin.devices}$`));
  await page.getByLabel(/Label/).fill("e2e iPad");
  await page.getByLabel("Max uses").fill("2");
  await page.getByRole("button", { name: "Generate enrol code" }).click();
  const code = page.getByTestId("enrol-code");
  await expect(code).toHaveText(/\S{4,}/);
  const text = (await code.innerText()).trim();
  await expect(page.getByRole("img", { name: `QR code for enrol code ${text}` })).toBeVisible();
  await expect(page.getByText(/2 uses/)).toBeVisible();
});

test("devices: revoke asks for confirmation and marks the device revoked", async ({ page }) => {
  const org = await loginToOverview(page);
  await page.goto(paths(org).admin.devices);
  const live = page.locator("table.data tbody tr", { has: page.getByRole("button", { name: "Revoke" }) });
  test.skip((await live.count()) === 0, "no live device to revoke");
  const before = await live.count();
  const row = live.first();
  const label = (await row.locator("td").first().innerText()).trim();
  await row.getByRole("button", { name: "Revoke" }).click();
  await row.getByRole("button", { name: "Confirm revoke" }).click();
  const revoked = page.locator("table.data tbody tr", { hasText: label });
  await expect(revoked.first().getByText("Revoked")).toBeVisible();
  await expect(live).toHaveCount(before - 1);
});

test("devices page renders in Nepali", async ({ page }) => {
  test.skip(LIVE, "locale flows use the mock backend's user locale seed");
  const org = await loginNe(page);
  await page.goto(paths(org).admin.devices);
  await expect(page.getByRole("heading", { level: 1 })).toHaveText("उपकरणहरू");
  await expect(page.getByRole("button", { name: "दर्ता कोड बनाउनुहोस्" })).toBeVisible();
});
