/** WP-C.15 TOTP enrolment (mock mode: password "demo", the mock accepts code 123456). */
import { expect, test, LIVE, loginToOverview } from "./helpers";

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
