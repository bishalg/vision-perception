import { expect, test } from "@playwright/test";
import { loginToOverview } from "./helpers";

test("sessions table -> session detail with Clips, Quality and Report tabs", async ({ page }) => {
  const org = await loginToOverview(page);
  await page.getByRole("navigation", { name: "Main navigation" }).getByRole("link", { name: "Sessions" }).click();
  await expect(page).toHaveURL(new RegExp(`/${org}/sessions$`));
  await expect(page.getByRole("heading", { name: "Sessions" })).toBeVisible();
  const rows = page.locator("table.data tbody tr");
  test.skip((await rows.count()) === 0, "no sessions in this org");

  const participant = (await rows.first().locator("td").nth(1).innerText()).trim();
  await rows.first().getByRole("link").click();
  await expect(page).toHaveURL(new RegExp(`/${org}/sessions/[^/?]+$`));
  await expect(page.getByRole("heading", { level: 1 })).toContainText(participant);
  for (const kpi of ["Composite", "Confidence", "Flag", "Quality", "Bundle"]) {
    await expect(page.getByRole("region", { name: kpi, exact: true })).toBeVisible();
  }

  const tabs = page.getByRole("navigation", { name: "Session sections" });
  await expect(tabs.getByRole("link", { name: "Clips" })).toHaveAttribute("aria-current", "page");
  await expect(page.locator("table.data").or(page.getByText("No per-clip scores recorded."))).toBeVisible();

  await tabs.getByRole("link", { name: "Quality" }).click();
  await expect(page).toHaveURL(/tab=quality/);
  await expect(page.getByText("Valid frame ratio")).toBeVisible();

  await tabs.getByRole("link", { name: "Report" }).click();
  await expect(page).toHaveURL(/tab=report/);
  await expect(
    page.getByRole("link", { name: "Download PDF report" }).or(page.getByText("No report is available for this session.")),
  ).toBeVisible();
});

test("filters live in the URL and narrow the table", async ({ page }) => {
  const org = await loginToOverview(page);
  await page.goto(`/${org}/sessions?flag=red`);
  await expect(page.getByLabel("Flag")).toHaveValue("red");
  const badges = page.locator("table.data tbody tr .badge");
  const n = await badges.count();
  for (let i = 0; i < n; i++) await expect(badges.nth(i)).toHaveText("Red");
});
