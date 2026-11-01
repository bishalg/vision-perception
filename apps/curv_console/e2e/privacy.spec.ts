import { expect, test } from "@playwright/test";
import { LIVE, loginToOverview } from "./helpers";

test.skip(LIVE, "mock-mode flows: they rely on the seeded mock erasure requests and participants");

const nav = (page: import("@playwright/test").Page) => page.getByRole("navigation", { name: "Main navigation" });

test("erasure requests: list with statuses and the held explanation", async ({ page }) => {
  const org = await loginToOverview(page);
  await nav(page).getByRole("link", { name: "Erasure requests" }).click();
  await expect(page).toHaveURL(new RegExp(`/${org}/erasure$`));
  await expect(page.getByRole("heading", { name: "Erasure requests" })).toBeVisible();
  await expect(page.getByTestId("held-explainer")).toContainText("kept under clinical record hold");
  const table = page.getByRole("table");
  await expect(table.getByText("Held", { exact: true })).toBeVisible();
  await expect(table.getByText("Completed", { exact: true }).first()).toBeVisible();
  await expect(table.getByText("Kept under clinical record hold")).toBeVisible();
  await expect(table.getByText(/sessions deleted/).first()).toBeVisible();
});

test("participants: request erasure after an in-page confirmation, then see the duplicate note", async ({ page }) => {
  const org = await loginToOverview(page);
  await nav(page).getByRole("link", { name: "Participants" }).click();
  await expect(page).toHaveURL(new RegExp(`/${org}/participants$`));
  const row = page.getByRole("row").nth(2);
  await row.getByRole("button", { name: /^Request erasure/ }).click();
  await expect(row.getByRole("alertdialog")).toContainText("Erase all data of participant");
  await row.getByRole("button", { name: "Cancel" }).click();
  await expect(row.getByRole("alertdialog")).toHaveCount(0);
  await row.getByRole("button", { name: /^Request erasure/ }).click();
  await row.getByRole("button", { name: "Yes, request erasure" }).click();
  await expect(row.getByText("Erasure requested.")).toBeVisible();
  await row.getByRole("button", { name: /^Request erasure/ }).click();
  await row.getByRole("button", { name: "Yes, request erasure" }).click();
  await expect(row.getByText(/already open/)).toBeVisible();
  await row.getByRole("link", { name: "View erasure requests" }).click();
  await expect(page).toHaveURL(new RegExp(`/${org}/erasure$`));
});

test("retention: preview then run the sweep and see the counts", async ({ page }) => {
  const org = await loginToOverview(page);
  await nav(page).getByRole("link", { name: "Data retention" }).click();
  await expect(page).toHaveURL(new RegExp(`/${org}/settings/retention$`));
  await page.getByRole("button", { name: "Preview (dry run)" }).click();
  await expect(page.getByTestId("sweep-result")).toContainText("would be deleted", { timeout: 15_000 });
  await page.getByRole("button", { name: "Run sweep" }).click();
  await expect(page.getByRole("alertdialog")).toBeVisible();
  await page.getByRole("button", { name: "Yes, run sweep" }).click();
  await expect(page.getByTestId("sweep-result")).toContainText("Sweep done", { timeout: 15_000 });
  await expect(page.getByTestId("sweep-result")).toContainText(/\d+ deleted/);
});

test("exports: a dataset is de-identified research only and can include labels", async ({ page }) => {
  const org = await loginToOverview(page);
  await nav(page).getByRole("link", { name: "Exports" }).click();
  await expect(page).toHaveURL(new RegExp(`/${org}/exports$`));
  await page.getByLabel("Format").selectOption("dataset_jsonl");
  await expect(page.getByLabel("Mode")).toBeDisabled();
  await expect(page.getByLabel("Mode").locator("option")).toHaveText(["De-identified"]);
  await expect(page.getByLabel("Include reviewer labels")).toBeChecked();
  await page.getByLabel(/^Reason/).fill("E2E research dataset for the study");
  await page.getByRole("button", { name: "Request export" }).click();
  await expect(page.getByText("Export queued.")).toBeVisible();
  await page.getByRole("link", { name: "View export" }).click();
  await expect(page.getByText("Research dataset (JSONL)").first()).toBeVisible();
});
