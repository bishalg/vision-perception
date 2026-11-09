/** WP-C.23 (spec 03 §3a): divisions in the top bar, the division's pages in the sidebar, and the old flat URLs. */
import { expect, test, LIVE, MOCK_ORG_ID, login, mainNav, navTo, paths, sideNav, OVERVIEW_URL } from "./helpers";

test.skip(LIVE, "mock-mode roles (admin@ and researcher@example.org)");

test("an old flat URL answers 308 with the new path, query kept (N-7)", async ({ request }) => {
  const old = await request.get(`/${MOCK_ORG_ID}/sessions?flag=red`, { maxRedirects: 0 });
  expect(old.status()).toBe(308);
  expect(old.headers()["location"]).toBe(`${paths(MOCK_ORG_ID).clinic.sessions}?flag=red`);
  const security = await request.get(`/${MOCK_ORG_ID}/settings/security`, { maxRedirects: 0 });
  expect(security.status()).toBe(308);
  expect(security.headers()["location"]).toBe(paths(MOCK_ORG_ID).account.security);
  const detail = await request.get(`/${MOCK_ORG_ID}/settings/integrations/abc`, { maxRedirects: 0 });
  expect(detail.headers()["location"]).toBe(paths(MOCK_ORG_ID).admin.integration("abc"));
});

test("admin: five divisions, the Clinic sidebar on the overview, an old link lands on the new page", async ({ page }) => {
  await login(page, "admin@example.org");
  await expect(page).toHaveURL(OVERVIEW_URL);
  await expect(mainNav(page).getByRole("link")).toHaveText(["Clinic", "Research", "Configuration", "Data", "Administration"]);
  await expect(mainNav(page).getByRole("link", { name: "Clinic" })).toHaveAttribute("aria-current", "true");
  await expect(sideNav(page, "Clinic").getByRole("link")).toHaveText(["Overview", "Worklist", "Participants", "Follow-ups", "Review", "Sessions"]);
  await expect(sideNav(page, "Clinic").getByRole("link", { name: "Overview" })).toHaveAttribute("aria-current", "page");

  await page.goto(`/${MOCK_ORG_ID}/audit`);
  await expect(page).toHaveURL(new RegExp(`${paths(MOCK_ORG_ID).data.audit}$`));
  await expect(sideNav(page, "Data").getByRole("link", { name: "Audit log" })).toHaveAttribute("aria-current", "page");
  await expect(page.getByRole("navigation", { name: "Breadcrumb" })).toContainText("Data");

  await navTo(page, "Administration", "Devices");
  await expect(page).toHaveURL(new RegExp(`${paths(MOCK_ORG_ID).admin.devices}$`));
});

test("researcher: the Research sidebar, no Administration", async ({ page }) => {
  await login(page, "researcher@example.org");
  await expect(page).toHaveURL(OVERVIEW_URL);
  await expect(mainNav(page).getByRole("link")).toHaveText(["Clinic", "Research", "Configuration", "Data"]);
  await mainNav(page).getByRole("link", { name: "Research", exact: true }).click();
  await expect(page).toHaveURL(new RegExp(`${paths(MOCK_ORG_ID).research.root}$`));
  await expect(sideNav(page, "Research").getByRole("link")).toHaveText(["Experiments", "Cohorts", "Analysis runs"]);
  await sideNav(page, "Research").getByRole("link", { name: "Analysis runs" }).click();
  await expect(page).toHaveURL(new RegExp(`${paths(MOCK_ORG_ID).research.runs}$`));
  await expect(sideNav(page, "Research").getByRole("link", { name: "Analysis runs" })).toHaveAttribute("aria-current", "page");
});

test("below 1024 px the sidebar is a drawer behind Menu; Escape closes it (N-3)", async ({ page }) => {
  await page.setViewportSize({ width: 800, height: 900 });
  await login(page, "admin@example.org");
  await expect(page).toHaveURL(OVERVIEW_URL);
  await expect(sideNav(page, "Clinic")).toBeHidden();
  const menu = page.getByRole("button", { name: "Menu" });
  await menu.click();
  await expect(sideNav(page, "Clinic")).toBeVisible();
  await expect(sideNav(page, "Clinic").getByRole("link", { name: "Overview" })).toBeFocused();
  await page.keyboard.press("Escape");
  await expect(sideNav(page, "Clinic")).toBeHidden();
  await expect(menu).toBeFocused();
});
