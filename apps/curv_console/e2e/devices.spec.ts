import { expect, test } from "@playwright/test";
import { loginToOverview } from "./helpers";

test("devices: generate an enrol code shown as text and QR", async ({ page }) => {
  const org = await loginToOverview(page);
  await page.getByRole("navigation", { name: "Main navigation" }).getByRole("link", { name: "Devices" }).click();
  await expect(page).toHaveURL(new RegExp(`/${org}/devices$`));
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
  await page.goto(`/${org}/devices`);
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
