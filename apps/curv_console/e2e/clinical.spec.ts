import { expect, test } from "@playwright/test";
import { LIVE, login, PASSWORD } from "./helpers";

// Mock mode picks the role from the email's local part; live runs need seeded clinicians, so these are mock-only.
test.skip(LIVE, "clinical flows use the mock backend's seeded worklist");
// Dev-mode first compiles of the session route can be slow on a busy machine.
test.describe.configure({ timeout: 240_000 });

const nav = (page: import("@playwright/test").Page) => page.getByRole("navigation", { name: "Main navigation" });

async function openWorklist(page: import("@playwright/test").Page, email: string): Promise<string> {
  await login(page, email);
  await expect(page).toHaveURL(/\/[0-9a-f-]{36}\/overview$/);
  const org = new URL(page.url()).pathname.split("/")[1]!;
  await nav(page).getByRole("link", { name: "Worklist" }).click();
  await expect(page).toHaveURL(new RegExp(`/${org}/worklist$`));
  await expect(page.getByRole("heading", { name: "Worklist" })).toBeVisible();
  return org;
}

test("worklist -> claim -> save draft -> sign (wrong password first) -> final and off the worklist", async ({ page }) => {
  const org = await openWorklist(page, "clinician@example.org");
  const row = page.locator("table.worklist-table tbody tr").filter({ hasText: "Not started" }).first();
  const link = row.getByRole("link");
  const href = (await link.getAttribute("href"))!;
  expect(href).toMatch(new RegExp(`^/${org}/sessions/[^/?]+\\?tab=clinical$`));
  await link.click();

  // The first visit compiles the session route in dev mode, which can be slow.
  await expect(page).toHaveURL(/tab=clinical/, { timeout: 90_000 });
  await expect(page.getByRole("navigation", { name: "Session sections" }).getByRole("link", { name: "Clinical review" })).toHaveAttribute("aria-current", "page");
  await page.getByRole("button", { name: "Claim review" }).click();
  await expect(page.getByTestId("review-state")).toHaveText("Claimed");

  // Nothing is pre-filled from the attention flag, and signing is blocked until a disposition is saved.
  await expect(page.getByLabel("Disposition")).toHaveValue("");
  await expect(page.getByRole("button", { name: "Sign", exact: true })).toBeDisabled();
  await page.getByLabel("Disposition").selectOption("monitor");
  await page.getByLabel("Impression").fill("Synthetic impression for the e2e run.");
  await page.getByRole("button", { name: "Save draft" }).click();
  await expect(page.getByText("Draft saved.")).toBeVisible();
  await expect(page.getByTestId("review-state")).toHaveText("In progress");

  await page.getByRole("button", { name: "Sign", exact: true }).click();
  const dialog = page.getByRole("dialog", { name: "Sign clinical review" });
  await expect(dialog.getByText("I have reviewed this observation record and approve this review.")).toBeVisible();
  await expect(dialog.getByTestId("hash-prefix")).toHaveText(/^[0-9a-f]{12}$/);

  await dialog.getByLabel("Your password").fill("wrong-password");
  await dialog.getByLabel(/I confirm the statement/).check();
  await dialog.getByRole("button", { name: "Sign review" }).click();
  await expect(dialog.getByRole("alert")).toContainText("not accepted");
  await expect(dialog.getByLabel("Your password")).toHaveValue("");

  await dialog.getByLabel("Your password").fill(PASSWORD);
  await dialog.getByLabel(/I confirm the statement/).check();
  await dialog.getByRole("button", { name: "Sign review" }).click();
  await expect(page.getByRole("dialog")).toHaveCount(0);
  await expect(page.getByTestId("review-state")).toHaveText("Final");
  await expect(page.getByText(/Dr\. Demo Clinician/)).toBeVisible();
  await expect(page.getByRole("button", { name: "Amend" })).toBeVisible();

  // The final review leaves the worklist.
  await page.goto(`/${org}/worklist`);
  await expect(page.locator(`a[href="${href}"]`)).toHaveCount(0);
});

test("a review awaiting a second signature can be cosigned by another clinician", async ({ page }) => {
  await openWorklist(page, "clinician@example.org");
  const row = page.locator("table.worklist-table tbody tr").filter({ hasText: "Awaiting cosign" }).first();
  await row.getByRole("link").click();
  await expect(page.getByTestId("review-state")).toHaveText("Awaiting cosign");
  await expect(page.getByRole("button", { name: "Save draft" })).toHaveCount(0);
  await page.getByRole("button", { name: "Cosign", exact: true }).click();
  const dialog = page.getByRole("dialog", { name: "Cosign clinical review" });
  await dialog.getByLabel("Your password").fill(PASSWORD);
  await dialog.getByLabel(/I confirm the statement/).check();
  await dialog.getByRole("button", { name: "Cosign review" }).click();
  await expect(page.getByTestId("review-state")).toHaveText("Final");
  await expect(page.getByText("Cosigned")).toBeVisible();
});

test("viewer sees no Worklist link and no Clinical review tab", async ({ page }) => {
  await login(page, "viewer@example.org");
  await expect(page).toHaveURL(/\/[0-9a-f-]{36}\/overview$/);
  await expect(nav(page).getByRole("link", { name: "Worklist" })).toHaveCount(0);
  await nav(page).getByRole("link", { name: "Sessions" }).click();
  await page.locator("table.data tbody tr").first().getByRole("link").click();
  await expect(page).toHaveURL(/\/sessions\/[^/?]+$/, { timeout: 90_000 });
  await expect(page.getByRole("navigation", { name: "Session sections" }).getByRole("link", { name: "Clips" })).toBeVisible();
  await expect(page.getByRole("navigation", { name: "Session sections" }).getByRole("link", { name: "Clinical review" })).toHaveCount(0);
  await page.goto(`${page.url().split("?")[0]}?tab=clinical`);
  await expect(page.getByText("Your role cannot view clinical reviews.")).toBeVisible();
});

test("admin sees the worklist but no clinical text, and can change clinical settings", async ({ page }) => {
  const org = await openWorklist(page, "admin@example.org");
  await page.locator("table.worklist-table tbody tr").first().getByRole("link").click();
  await expect(page.getByRole("button", { name: "Claim review" })).toHaveCount(0);

  await page.goto(`/${org}/settings/clinical`);
  await expect(page.getByRole("heading", { name: "Clinical settings", level: 1 })).toBeVisible();
  await page.getByLabel("Second signature (cosign)").selectOption("none");
  await page.getByLabel("Paper size").selectOption("letter");
  await page.getByRole("button", { name: "Save organisation settings" }).click();
  await expect(page.getByText("Settings saved.")).toBeVisible();
  await page.getByLabel("Display name").fill("Dr. Admin Test");
  await page.getByRole("button", { name: "Save profile" }).click();
  await expect(page.getByText("Profile saved.")).toBeVisible();
});
