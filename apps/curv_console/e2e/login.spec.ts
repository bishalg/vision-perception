import { expect, test } from "@playwright/test";
import { MOCK_ORG_ID, SUPERADMIN_EMAIL, SUPERADMIN_PASSWORD, TARGET_ORG_ID, login, loginToOverview } from "./helpers";

test("unauthenticated visits go to /login and keep the target", async ({ page }) => {
  await page.goto("/");
  await expect(page).toHaveURL(/\/login$/);
  await page.goto(`/${MOCK_ORG_ID}/sessions?flag=red`);
  await expect(page).toHaveURL(/\/login\?next=/);
});

test("wrong password shows an error and sets no session", async ({ page }) => {
  await login(page, "demo@example.org", "definitely-wrong");
  // Next also renders an empty route-announcer alert, so match the form's alert by text.
  await expect(page.getByRole("alert").filter({ hasText: "Invalid email or password." })).toBeVisible();
  await expect(page).toHaveURL(/\/login/);
});

test("login lands on the overview KPIs; the session cookie is httpOnly; sign out ends it", async ({ page, context }) => {
  await loginToOverview(page);
  await expect(page.getByRole("heading", { name: "Overview" })).toBeVisible();
  for (const kpi of ["Sessions (7 days)", "Sessions (30 days)", "Review queue", "Active devices"]) {
    await expect(page.getByRole("region", { name: kpi })).toBeVisible();
  }
  await expect(page.getByRole("region", { name: "Attention flags (30 days)" })).toBeVisible();
  const cookies = await context.cookies();
  const access = cookies.find((c) => c.name === "curv_access");
  expect(access?.httpOnly).toBe(true);
  expect(access?.sameSite).toBe("Lax");
  expect(await page.evaluate(() => document.cookie)).not.toContain("curv_access");

  await page.getByRole("button", { name: "Sign out" }).click();
  await expect(page).toHaveURL(/\/login$/);
  await page.goto("/");
  await expect(page).toHaveURL(/\/login$/);
});

test("the access cookie expiring triggers a silent refresh", async ({ page, context }) => {
  const org = await loginToOverview(page);
  await context.clearCookies({ name: "curv_access" });
  await page.goto(`/${org}/sessions`);
  await expect(page.getByRole("heading", { name: "Sessions" })).toBeVisible();
  expect((await context.cookies()).some((c) => c.name === "curv_access")).toBe(true);
});

test("superadmin (no home org) picks an org by id, then sees it in the switcher", async ({ page }) => {
  test.skip(!SUPERADMIN_EMAIL, "live run without E2E_SUPERADMIN_EMAIL");
  await login(page, SUPERADMIN_EMAIL, SUPERADMIN_PASSWORD);
  await expect(page.getByRole("heading", { name: "Choose an organisation" })).toBeVisible();
  await page.getByLabel("Organisation id").fill("not-an-id");
  await page.getByRole("button", { name: "Switch" }).click();
  await expect(page.getByRole("alert").filter({ hasText: "That is not a valid organisation id." })).toBeVisible();
  await page.getByLabel("Organisation id").fill(TARGET_ORG_ID);
  await page.getByRole("button", { name: "Switch" }).click();
  await expect(page).toHaveURL(new RegExp(`/${TARGET_ORG_ID}/overview$`));
  await expect(page.getByRole("combobox", { name: "Organisation", exact: true })).toHaveValue(TARGET_ORG_ID);
  await expect(page.getByRole("textbox", { name: "Organisation id" })).toBeVisible();
});
