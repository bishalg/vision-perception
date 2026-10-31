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
  // Flag badges only: the tracking column has its own badges (`badge-trk-*`).
  const badges = page.locator("table.data tbody tr .badge:not([class*='badge-trk'])");
  const n = await badges.count();
  for (let i = 0; i < n; i++) await expect(badges.nth(i)).toHaveText("Red");
});

test("tracking filter narrows the table; Quality tab shows the tracking section", async ({ page }) => {
  const org = await loginToOverview(page);
  await page.goto(`/${org}/sessions?tracking=poor`);
  await expect(page.getByLabel("Tracking")).toHaveValue("poor");
  const rows = page.locator("table.data tbody tr");
  test.skip((await rows.count()) === 0, "no poor-tracking sessions in this org");
  await expect(rows.locator(".badge-trk-poor")).toHaveCount(await rows.count());

  await rows.first().getByRole("link").click();
  await page.getByRole("navigation", { name: "Session sections" }).getByRole("link", { name: "Quality" }).click();
  await expect(page.getByRole("heading", { name: "Tracking", exact: true })).toBeVisible();
  await expect(page.getByText("Tracking verdict")).toBeVisible();
  await expect(page.getByText("Estimated accuracy")).toBeVisible();
  await expect(page.getByRole("img", { name: /Gaze offset in degrees/ })).toBeVisible();
  await expect(page.getByRole("heading", { name: "Data loss by track state" })).toBeVisible();
});
