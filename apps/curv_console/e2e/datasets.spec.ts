/**
 * WP-X.11 research datasets in mock mode (ADR-0015 R-7..R-9): the Datasets item in the Research sidebar; freezing a
 * dataset from a cohort with exclusion rules, labels and a seeded split; its page shows the content hash, lineage,
 * members, exclusions with reasons and the data dictionary; "Start run from this dataset" opens the experiment's run
 * dialog on the dataset and the run records the dataset hash.
 */
import { expect, test, LIVE, login, navTo, OVERVIEW_URL, paths, MOCK_ORG_ID } from "./helpers";

test.skip(LIVE, "mock-mode flow: it relies on the seeded mock research data");

test("create a dataset from a cohort, see its hash and members, run from it", async ({ page }) => {
  await login(page, "researcher@example.org");
  await expect(page).toHaveURL(OVERVIEW_URL);
  await navTo(page, "Research", "Datasets");
  await expect(page.getByRole("heading", { name: "Datasets", exact: true })).toBeVisible();
  await expect(page.getByRole("table", { name: "Datasets" })).toContainText("Toddlers v1");

  // Freeze a new dataset from the toddlers cohort.
  await page.getByRole("link", { name: "New dataset" }).click();
  await expect(page.getByRole("heading", { name: "New dataset" })).toBeVisible();
  const name = `E2E dataset ${Date.now()}`;
  await page.getByLabel("Cohort").selectOption({ label: "24-47 months, good/fair tracking" });
  await page.getByLabel("Name", { exact: true }).fill(name);
  await page.getByLabel("Minimum valid-frame ratio (0 to 1, optional)").fill("0.6");
  await page.getByLabel("Leave out these sessions").fill("SESS-MOCK-0005");
  await page.getByLabel("Reason for leaving them out").fill("recorded twice");
  await page.getByLabel("Attach clinician labels").check();
  await page.getByLabel("Split into train, validation and holdout").check();
  await page.getByLabel("Split seed (optional)").fill("9");
  await page.getByRole("button", { name: "Freeze dataset" }).click();

  // The dataset page: hash, facts, split, members, exclusions, dictionary.
  await expect(page.getByRole("heading", { name })).toBeVisible({ timeout: 45_000 });
  const hash = page.getByTestId("dataset-hash");
  await expect(hash).toHaveAttribute("data-hash", /^[0-9a-f]{64}$/);
  const full = (await hash.getAttribute("data-hash"))!;
  await expect(hash.locator("code").first()).toHaveText(`${full.slice(0, 12)}…`);
  await expect(page.getByRole("region", { name: "Lineage" })).toContainText("asd-backend/");
  await expect(page.getByText("Seed 9, stratified by age band")).toBeVisible();
  await expect(page.getByRole("table", { name: "Split" }).locator('[data-split-row="total"]')).toBeVisible();
  const members = page.getByRole("table", { name: "Members" });
  await expect(members.locator("tbody tr")).not.toHaveCount(0);
  await expect(members).toContainText("P-");
  await expect(members.locator('[data-member="SESS-MOCK-0005"]')).toHaveCount(0);
  const excluded = page.getByRole("table", { name: "Exclusions" });
  await expect(excluded.locator('[data-exclusion="SESS-MOCK-0005"]')).toContainText("recorded twice");
  await expect(excluded.locator('[data-exclusion="SESS-MOCK-NOAGE"]')).toContainText("No placeable age");
  await expect(page.getByRole("table", { name: "Data dictionary" })).toContainText("participant_pseudonym");
  const datasetId = new URL(page.url()).pathname.split("/").pop()!;
  // The export pre-fill links to the exports page with this dataset.
  await expect(page.getByRole("link", { name: "Export this dataset" })).toHaveAttribute(
    "href",
    paths(MOCK_ORG_ID).data.exportDataset(datasetId),
  );

  // Start a run from it: the experiment's run dialog opens on this dataset.
  await page.getByRole("combobox", { name: "Experiment" }).selectOption({ label: "Lower I-VT threshold" });
  await page.getByRole("button", { name: "Start run from this dataset" }).click();
  await expect(page).toHaveURL((u) => u.searchParams.get("dataset") === datasetId, { timeout: 45_000 });
  const dialog = page.getByRole("dialog", { name: "Run on cohort" });
  await expect(dialog).toBeVisible({ timeout: 45_000 });
  await expect(dialog.getByLabel("A frozen dataset")).toBeChecked();
  await expect(dialog.getByRole("combobox", { name: "Dataset" })).toHaveValue(datasetId);
  await expect(dialog.getByLabel("Seed (optional)")).toHaveValue("9");
  await dialog.getByRole("button", { name: "Start run" }).click();
  await expect(page).toHaveURL(new RegExp(`${paths(MOCK_ORG_ID).research.runs}/[0-9a-f-]+$`), { timeout: 45_000 });
  await expect(page.locator('[data-provenance="dataset_hash"]')).toContainText(full);
});
