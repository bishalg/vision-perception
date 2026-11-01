import { expect, test, type Page } from "@playwright/test";
import { LIVE, login } from "./helpers";

// Mock mode picks the role from the email's local part and seeds notes, disclosures and MRNs on the newest participant.
test.skip(LIVE, "participant workspace flows use the mock backend's seeded participants");
test.describe.configure({ timeout: 240_000 });

async function openFirstParticipant(page: Page, email: string): Promise<{ org: string; base: string }> {
  await login(page, email);
  await expect(page).toHaveURL(/\/[0-9a-f-]{36}\/overview$/);
  const org = new URL(page.url()).pathname.split("/")[1]!;
  await page.goto(`/${org}/participants`);
  // The newest participant is the first row and carries the seeded notes and disclosure.
  const link = page.locator("table.data tbody tr").first().getByRole("link").first();
  await link.click();
  await expect(page).toHaveURL(new RegExp(`/${org}/participants/[0-9a-f-]{36}$`), { timeout: 90_000 });
  return { org, base: new URL(page.url()).pathname };
}

test("clinician: header and timeline filter", async ({ page }) => {
  await openFirstParticipant(page, "clinician@example.org");
  await expect(page.getByRole("heading", { level: 1 })).toContainText("child-");
  const tabs = page.getByRole("navigation", { name: "Participant sections" });
  await expect(tabs.getByRole("link")).toHaveText(["Timeline", "Notes", "MRN", "Letters", "Disclosures", "Print summary"]);
  const rows = page.locator("table.data tbody tr");
  await expect(rows.first()).toBeVisible();
  expect(await rows.count()).toBeGreaterThan(3);
  await page.getByRole("checkbox", { name: "Note" }).check();
  await page.getByRole("button", { name: "Apply filter" }).click();
  await expect(page).toHaveURL(/kind=note/);
  await expect(page.locator("table.data tbody tr")).not.toHaveCount(0);
  for (const kind of await page.locator("table.data tbody tr").evaluateAll((els) => els.map((e) => e.getAttribute("data-kind")))) {
    expect(kind).toBe("note");
  }
  // The filter is applied by the server, so no page-only caveat is shown.
  await expect(page.getByText(/apply to the entries on this page/)).toHaveCount(0);
  await page.getByRole("link", { name: "Show all" }).click();
  await expect(page).toHaveURL(/tab=timeline$/);
  // A date range filters on the server too: an empty far-past range shows the empty message.
  await page.getByLabel("From").fill("2001-01-01");
  await page.getByLabel("To", { exact: true }).fill("2001-01-31");
  await page.getByRole("button", { name: "Apply filter" }).click();
  await expect(page).toHaveURL(/from=2001-01-01&to=2001-01-31|to=2001-01-31&from=2001-01-01/);
  await expect(page.getByText("No entries match the chosen filters.")).toBeVisible();
});

test("clinician: notes show a struck-through original, add a note and correct it", async ({ page }) => {
  await openFirstParticipant(page, "clinician@example.org");
  await page.getByRole("link", { name: "Notes", exact: true }).click();
  await expect(page.getByRole("heading", { name: "Clinical notes" })).toBeVisible();
  await expect(page.locator('li.note[data-corrected="true"] s').first()).toContainText("good eye contact");

  await page.getByLabel("Note", { exact: true }).fill("Follow-up call booked for next week");
  await page.getByRole("button", { name: "Add note" }).click();
  const added = page.locator("li.note").filter({ hasText: "Follow-up call booked" });
  await expect(added).toBeVisible();

  await added.getByRole("button", { name: /^Correct:/ }).click();
  await added.getByRole("textbox", { name: "Correct this note" }).fill("Follow-up call booked for Monday");
  await added.getByRole("button", { name: "Save correction" }).click();
  await expect(page.locator('li.note[data-corrected="true"] s').filter({ hasText: "next week" })).toBeVisible();
  await expect(page.locator("li.note").filter({ hasText: "booked for Monday" })).toBeVisible();
});

test("clinician: MRN is entered by form, shown on request and never in the URL", async ({ page }) => {
  const { base } = await openFirstParticipant(page, "clinician@example.org");
  await page.getByRole("link", { name: "MRN", exact: true }).click();
  await expect(page.getByRole("heading", { name: "Hospital patient reference (MRN)" })).toBeVisible();
  if (await page.getByTestId("mrn-on-file").isVisible()) {
    await page.getByRole("button", { name: "Unlink MRN" }).click();
    await page.getByRole("button", { name: "Yes, unlink" }).click();
  }
  await expect(page.getByTestId("mrn-none")).toBeVisible();
  const link = page.getByRole("form", { name: "Link an MRN" });
  await link.getByLabel("Assigning authority").fill("tuth");
  await link.getByLabel("MRN").fill("12-345");
  await link.getByRole("button", { name: "Save MRN" }).click();
  await expect(page.getByTestId("mrn-on-file")).toBeVisible();
  await expect(page.getByTestId("mrn-value")).toHaveCount(0);
  await page.getByRole("button", { name: "Show MRN" }).click();
  await expect(page.getByTestId("mrn-value")).toHaveText("12-345");
  expect(page.url()).not.toContain("12-345");

  const search = page.getByRole("form", { name: "Find a participant by MRN" });
  await search.getByLabel("Assigning authority").fill("TUTH");
  await search.getByLabel("MRN").fill("12345");
  // The hit redirects to the page we are already on, so the URL alone does not show the search is done: wait for the
  // server action and its navigation, or a late re-render resets the unlink confirmation below.
  const searched = page.waitForResponse((r) => r.request().method() === "POST" && r.request().headers()["next-action"] !== undefined);
  await search.getByRole("button", { name: "Search" }).click();
  await searched;
  await expect(search.getByRole("button", { name: "Search" })).toBeEnabled();
  await expect(page).toHaveURL(new RegExp(`${base}\\?tab=mrn$`));
  expect(page.url()).not.toContain("12345");
  await expect(page.getByTestId("mrn-on-file")).toBeVisible();

  await page.getByRole("button", { name: "Unlink MRN" }).click();
  await page.getByRole("button", { name: "Yes, unlink" }).click();
  await expect(page.getByTestId("mrn-none")).toBeVisible();
});

test("clinician: disclosures, letters placeholder and the print view", async ({ page }) => {
  await openFirstParticipant(page, "clinician@example.org");
  await page.getByRole("link", { name: "Disclosures", exact: true }).click();
  await expect(page.getByRole("table").getByText("Clinical care")).toBeVisible();
  await expect(page.getByText("Hospital SFTP (test): delivered")).toBeVisible();

  await page.getByRole("link", { name: "Letters", exact: true }).click();
  await expect(page.getByTestId("letters-soon")).toContainText("coming soon");

  await page.getByRole("link", { name: "Print summary" }).click();
  await expect(page.getByRole("button", { name: "Print" })).toBeVisible();
  await expect(page.getByRole("table").first()).toContainText("Signed by");
  await expect(page.getByRole("heading", { name: "Home care activity" })).toBeVisible();
  await expect(page.getByText(/care events recorded/)).toBeVisible();
  await page.emulateMedia({ media: "print" });
  await expect(page.locator(".topbar")).toBeHidden();
  await expect(page.getByRole("button", { name: "Print" })).toBeHidden();
  await expect(page.getByRole("heading", { name: "Participant clinical summary" })).toBeVisible();
  await page.emulateMedia({ media: "screen" });
});

test("a viewer sees the timeline and the print summary only, without clinical content", async ({ page }) => {
  await openFirstParticipant(page, "viewer@example.org");
  const tabs = page.getByRole("navigation", { name: "Participant sections" });
  await expect(tabs.getByRole("link")).toHaveText(["Timeline", "Print summary"]);
  await page.goto(`${new URL(page.url()).pathname}?tab=notes`);
  await expect(page.getByRole("heading", { name: "Timeline" })).toBeVisible();
  await page.getByRole("link", { name: "Print summary" }).click();
  await expect(page.getByText("Not shown for your role").first()).toBeVisible();
  await expect(page.getByText("Clinical notes")).toHaveCount(0);
  await expect(page.getByText("Home care activity")).toHaveCount(0);
});
