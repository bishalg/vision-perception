/** WP-C.15 TOTP enrolment (mock mode: password "demo", the mock accepts code 123456). */
import { expect, test, LIVE, MOCK_ORG_ID, login, loginToOverview } from "./helpers";

test.skip(LIVE, "the mock backend's fixed TOTP code");

const CODE = "123456";

test("enrol -> confirm -> enrolled, then turn off", async ({ page }) => {
  const org = await loginToOverview(page);
  await page.getByRole("navigation", { name: "Main navigation" }).getByRole("link", { name: "Account security" }).click();
  await expect(page).toHaveURL(new RegExp(`/${org}/settings/security$`));
  await expect(page.getByRole("heading", { name: "Account security", level: 1 })).toBeVisible();

  const setup = page.getByRole("region", { name: "Set up two-step sign-in" });
  await setup.getByLabel("Current password").fill("wrong");
  await setup.getByRole("button", { name: "Get a key" }).click();
  await expect(setup.getByRole("alert")).toHaveText("The password is not correct.");

  await setup.getByLabel("Current password").fill("demo");
  await setup.getByRole("button", { name: "Get a key" }).click();
  await expect(setup.getByTestId("totp-secret")).toHaveText(/^[A-Z2-7]{32}$/);
  await expect(setup.getByRole("img", { name: "QR code for your authenticator app" })).toBeVisible();

  await setup.getByLabel("Code from your authenticator app").fill("000000");
  await setup.getByRole("button", { name: "Confirm" }).click();
  await expect(setup.getByRole("alert")).toContainText("That code is not correct");
  await expect(setup.getByTestId("totp-secret")).toBeVisible();

  await setup.getByLabel("Code from your authenticator app").fill(CODE);
  await setup.getByRole("button", { name: "Confirm" }).click();
  await expect(page.getByRole("status")).toHaveText(/Two-step sign-in is on/);
  await expect(setup.getByTestId("totp-secret")).toHaveCount(0);

  // A second setup is refused while TOTP is on.
  await setup.getByLabel("Current password").fill("demo");
  await setup.getByRole("button", { name: "Get a key" }).click();
  await expect(setup.getByRole("alert")).toContainText("already on");

  const off = page.getByRole("region", { name: "Turn off two-step sign-in" });
  await off.getByLabel("Current password").fill("demo");
  await off.getByLabel("Code from your authenticator app").fill(CODE);
  await off.getByRole("button", { name: "Turn off" }).click();
  await expect(page.getByRole("status")).toHaveText("Two-step sign-in is off.");

  // Off again: the backend says it is not on.
  await off.getByLabel("Current password").fill("demo");
  await off.getByLabel("Code from your authenticator app").fill(CODE);
  await off.getByRole("button", { name: "Turn off" }).click();
  await expect(off.getByRole("alert")).toContainText("not on");
});

test.describe("mandatory two-step sign-in (WP-C.17, acceptance 26)", () => {
  test("grace: the overview nudge shows the deadline and nothing is locked", async ({ page }) => {
    await login(page, "grace@example.org");
    await expect(page).toHaveURL(/\/[0-9a-f-]{36}\/overview$/);
    await expect(page.getByTestId("totp-nudge")).toContainText(/Set it up by .*2026/);
    // Grace never locks the user out.
    await page.goto(`/${MOCK_ORG_ID}/sessions`);
    await expect(page.getByRole("heading", { name: "Sessions", level: 1 })).toBeVisible();
  });

  test("a viewer never sees the nudge", async ({ page }) => {
    await login(page, "viewer@example.org");
    await expect(page).toHaveURL(/\/[0-9a-f-]{36}\/overview$/);
    await expect(page.getByRole("heading", { name: "Overview", level: 1 })).toBeVisible();
    await expect(page.getByTestId("totp-nudge")).toHaveCount(0);
  });

  test("required: redirected to the security page, enrols, then the overview renders", async ({ page }) => {
    await login(page, "required@example.org");
    const security = new RegExp(`/${MOCK_ORG_ID}/settings/security\\?required=1$`);
    await expect(page).toHaveURL(security);
    await expect(page.getByTestId("totp-required")).toContainText("Your role requires two-step sign-in");

    // Every other page redirects back (the mock answers 403 totp-enrolment-required for it).
    for (const path of ["overview", "sessions"]) {
      await page.goto(`/${MOCK_ORG_ID}/${path}`);
      await expect(page).toHaveURL(security);
    }

    const setup = page.getByRole("region", { name: "Set up two-step sign-in" });
    await setup.getByLabel("Current password").fill("demo");
    await setup.getByRole("button", { name: "Get a key" }).click();
    await expect(setup.getByTestId("totp-secret")).toHaveText(/^[A-Z2-7]{32}$/);
    await setup.getByLabel("Code from your authenticator app").fill(CODE);
    await setup.getByRole("button", { name: "Confirm" }).click();
    await expect(page.getByRole("status")).toHaveText(/Two-step sign-in is on/);
    await expect(page.getByTestId("totp-required")).toHaveCount(0);

    await page.goto(`/${MOCK_ORG_ID}/overview`);
    await expect(page).toHaveURL(new RegExp(`/${MOCK_ORG_ID}/overview$`));
    await expect(page.getByRole("heading", { name: "Overview", level: 1 })).toBeVisible();
    await expect(page.getByTestId("totp-nudge")).toHaveCount(0);
  });
});

test.describe("automatic logoff (WP-C.21, acceptance 23)", () => {
  test.skip(LIVE, "needs to rewrite the console's stamp cookies");

  async function stamp(page: import("@playwright/test").Page, name: string, ageMs: number) {
    const url = new URL(page.url()).origin;
    await page.context().addCookies([{ name, value: String(Date.now() - ageMs), url, httpOnly: true, sameSite: "Lax" }]);
  }

  test("idle for 31 minutes: the next page load lands on /login?reason=idle", async ({ page }) => {
    const org = await loginToOverview(page);
    await stamp(page, "curv_last_seen", 31 * 60_000);
    await page.goto(`/${org}/sessions`);
    await expect(page).toHaveURL(/\/login\?reason=idle/);
    await expect(page.getByTestId("logoff-reason")).toContainText("idle for 30 minutes");
    // The auth cookies are gone: a protected page now needs a fresh login.
    await page.goto(`/${org}/overview`);
    await expect(page).toHaveURL(/\/login/);
  });

  test("an idle tab's client-side navigation lands on /login?reason=idle (wave 1 defect 1)", async ({ page }) => {
    const org = await loginToOverview(page);
    // Prove the click below is a client-side (RSC) navigation: a full load would reset this marker.
    await page.evaluate(() => ((window as unknown as { __spa: boolean }).__spa = true));
    const nav = page.getByRole("navigation", { name: "Main navigation" });
    await nav.getByRole("link", { name: "Sessions" }).click();
    await expect(page).toHaveURL(new RegExp(`/${org}/sessions`));
    expect(await page.evaluate(() => (window as unknown as { __spa?: boolean }).__spa)).toBe(true);

    await stamp(page, "curv_last_seen", 31 * 60_000);
    await nav.getByRole("link", { name: "Overview" }).click();
    await expect(page).toHaveURL(/\/login\?reason=idle/);
    await expect(page.getByTestId("logoff-reason")).toContainText("idle for 30 minutes");
    const names = (await page.context().cookies()).map((c) => c.name);
    expect(names).not.toContain("curv_access");
    expect(names).not.toContain("curv_refresh");
    await page.goto(`/${org}/overview`);
    await expect(page).toHaveURL(/\/login/);
  });

  test("an idle tab's server action does not run and lands on /login?reason=idle", async ({ page }) => {
    const org = await loginToOverview(page);
    await page.goto(`/${org}/settings/security`);
    const setup = page.getByRole("region", { name: "Set up two-step sign-in" });
    await setup.getByLabel("Current password").fill("demo");
    await stamp(page, "curv_last_seen", 31 * 60_000);
    await setup.getByRole("button", { name: "Get a key" }).click();
    await expect(page).toHaveURL(/\/login\?reason=idle/);
    await expect(page.getByTestId("logoff-reason")).toContainText("idle for 30 minutes");
    await expect(page.getByTestId("totp-secret")).toHaveCount(0);
  });

  test("an active session older than 12 hours lands on /login?reason=expired", async ({ page }) => {
    const org = await loginToOverview(page);
    await stamp(page, "curv_login_at", 13 * 3_600_000);
    await page.goto(`/${org}/sessions`);
    await expect(page).toHaveURL(/\/login\?reason=expired/);
    await expect(page.getByTestId("logoff-reason")).toContainText("12-hour limit");
  });

  test("a recent session is untouched", async ({ page }) => {
    const org = await loginToOverview(page);
    await stamp(page, "curv_last_seen", 5 * 60_000);
    await page.goto(`/${org}/sessions`);
    await expect(page.getByRole("heading", { name: "Sessions", level: 1 })).toBeVisible();
  });
});
