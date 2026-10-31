/**
 * Live console smoke, driven by scripts/live_smoke.sh after it has ingested a 1.6 and a 1.7
 * session and signed the 1.7 one: the clinician logs in, finds the 1.7 session in the
 * sessions list, opens its detail (Quality tab with tracking, Report tab), downloads the
 * report through the console's /api/report proxy, then opens the worklist, which still holds
 * the unsigned 1.6 session.
 *
 * The Report tab must offer the download for a live (TrueDepth) capture: the server renders
 * the report from the stored record, whatever `report_pdf_available` (device upload) says.
 */
import { expect, test } from "@playwright/test";

const env = (name: string): string => {
  const v = process.env[name];
  if (!v) throw new Error(`${name} is required (set by scripts/live_smoke.sh)`);
  return v;
};

test("clinician: login -> sessions list -> 1.7 detail (Quality, Report) -> worklist", async ({ page }) => {
  const org = env("E2E_ORG_ID");
  const s17 = env("E2E_SESSION_17");
  const s16 = env("E2E_SESSION_16");

  await page.goto("/login");
  await page.getByLabel("Email").fill(env("E2E_EMAIL"));
  await page.getByLabel("Password").fill(env("E2E_PASSWORD"));
  await page.getByRole("button", { name: "Sign in" }).click();
  await expect(page).toHaveURL(new RegExp(`/${org}/overview$`), { timeout: 120_000 });

  // Sessions list holds the ingested 1.7 session; open it.
  await page.goto(`/${org}/sessions`);
  await expect(page.getByRole("heading", { name: "Sessions", level: 1 })).toBeVisible();
  const link = page.locator(`table.data tbody tr a[href^="/${org}/sessions/${s17}"]`).first();
  await expect(link).toBeVisible();
  await link.click();
  await expect(page).toHaveURL(new RegExp(`/${org}/sessions/${s17}`), { timeout: 120_000 });
  await expect(page.getByRole("region", { name: "Composite", exact: true })).toBeVisible();

  // Quality tab: the 1.7 tracking-quality record is rendered.
  const tabs = page.getByRole("navigation", { name: "Session sections" });
  await tabs.getByRole("link", { name: "Quality" }).click();
  await expect(page).toHaveURL(/tab=quality/);
  await expect(page.getByRole("heading", { name: "Tracking", exact: true })).toBeVisible();
  await expect(page.getByText("Tracking verdict")).toBeVisible();

  // Report tab, then the console's report proxy (server-rendered PDF from the backend).
  await tabs.getByRole("link", { name: "Report" }).click();
  await expect(page).toHaveURL(/tab=report/);
  await expect(page.getByRole("link", { name: "Download PDF report" })).toBeVisible();
  const pdf = await page.request.get(`/api/report/${encodeURIComponent(s17)}`);
  expect(pdf.status()).toBe(200);
  expect((await pdf.body()).subarray(0, 4).toString()).toBe("%PDF");

  // Worklist: the unsigned 1.6 session is waiting.
  await page.goto(`/${org}/worklist`);
  await expect(page.getByRole("heading", { name: "Worklist", level: 1 })).toBeVisible();
  await expect(page.locator(`a[href^="/${org}/sessions/${s16}"]`).first()).toBeVisible();
});
