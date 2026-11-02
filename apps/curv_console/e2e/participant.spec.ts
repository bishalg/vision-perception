import { type Page } from "@playwright/test";
import { expect, test, LIVE, login } from "./helpers";

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
  await expect(tabs.getByRole("link")).toHaveText([
    "Timeline", "Visits", "Files", "Notes", "MRN", "Letters", "Disclosures", "Print summary",
  ]);
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
  await expect(tabs.getByRole("link")).toHaveText(["Timeline", "Visits", "Print summary"]);
  await page.goto(`${new URL(page.url()).pathname}?tab=notes`);
  await expect(page.getByRole("heading", { name: "Timeline" })).toBeVisible();
  await page.getByRole("link", { name: "Print summary" }).click();
  await expect(page.getByText("Not shown for your role").first()).toBeVisible();
  await expect(page.getByText("Clinical notes")).toHaveCount(0);
  await expect(page.getByText("Home care activity")).toHaveCount(0);
});

// ---- WP-C.18 record workspace (visits, files, follow-ups, search, new participant, TOTP reset) ----------------

const mainNav = (page: Page) => page.getByRole("navigation", { name: "Main navigation" });

/** UI-4: nothing typed into the record ends up in the address bar or the title. */
async function expectNotLeaked(page: Page, ...secrets: string[]) {
  const title = await page.title();
  for (const s of secrets) {
    expect(page.url()).not.toContain(encodeURIComponent(s));
    expect(page.url()).not.toContain(s);
    expect(title).not.toContain(s);
  }
}

test("clinician: add a visit with a summary and a follow-up, link a session, see it on the timeline, close it", async ({ page }) => {
  await openFirstParticipant(page, "clinician@example.org");
  await page.getByRole("navigation", { name: "Participant sections" }).getByRole("link", { name: "Visits" }).click();
  await expect(page.getByRole("heading", { name: "Visits", level: 2 })).toBeVisible();
  // Seeded history: a closed observation visit whose summary a clinician can read.
  await expect(page.getByTestId("visit-summary").first()).toBeVisible();

  await page.getByRole("button", { name: "Add a visit" }).click();
  const form = page.getByRole("form", { name: "New visit" });
  await form.getByLabel("Type of visit").selectOption("home_visit");
  await form.getByLabel("Reason for the visit (short)").fill("Home routine check");
  await form.getByLabel("Visit summary").fill("Sleeps well; plays alone in the evenings");
  await form.getByLabel("Follow up by (optional)").fill("2099-01-31");
  await form.getByRole("checkbox").first().check();
  await form.getByRole("button", { name: "Save visit" }).click();
  await expect(page.getByRole("status")).toHaveText("Visit saved.");
  const visit = page.getByTestId("visit").filter({ hasText: "Home routine check" });
  await expect(visit).toBeVisible();
  await expect(visit.getByTestId("visit-summary")).toHaveText("Sleeps well; plays alone in the evenings");
  await expect(visit.getByRole("link")).toHaveCount(1); // the linked session
  await expectNotLeaked(page, "Sleeps well", "Home routine check");

  await page.getByRole("navigation", { name: "Participant sections" }).getByRole("link", { name: "Timeline" }).click();
  await expect(page.locator('table.data tbody tr[data-kind="encounter"]').filter({ hasText: "Home visit (open)" })).toBeVisible();
  await expect(page.getByText("Sleeps well")).toHaveCount(0);

  await page.getByRole("navigation", { name: "Participant sections" }).getByRole("link", { name: "Visits" }).click();
  await visit.getByRole("button", { name: "Close visit" }).click();
  await expect(page.getByTestId("visit").filter({ hasText: "Home routine check" })).toHaveAttribute("data-status", "closed");
  // A clinician may reopen (REC-4).
  await expect(page.getByTestId("visit").filter({ hasText: "Home routine check" }).getByRole("button", { name: "Reopen visit" })).toBeVisible();
});

test("admin: no summary field on visits; uploads a file but cannot open it", async ({ page }) => {
  await openFirstParticipant(page, "admin@example.org");
  const tabs = page.getByRole("navigation", { name: "Participant sections" });
  await expect(tabs.getByRole("link")).toHaveText(["Timeline", "Visits", "Files", "MRN", "Print summary"]);
  await tabs.getByRole("link", { name: "Visits" }).click();
  await expect(page.getByText("A summary is on file. Your role cannot read it.").first()).toBeVisible();
  await page.getByRole("button", { name: "Add a visit" }).click();
  await expect(page.getByLabel("Visit summary")).toHaveCount(0);
  await expect(page.getByText("Only clinicians write visit summaries.")).toBeVisible();
  await page.getByRole("button", { name: "Cancel" }).click();

  await tabs.getByRole("link", { name: "Files" }).click();
  await expect(page.getByRole("heading", { name: "Files", level: 2 })).toBeVisible();
  const upload = page.getByRole("form", { name: "Add a file" });
  // 11 MB is refused in the browser, with the API's words, before anything is sent.
  await upload.getByLabel(/Choose a file/).setInputFiles({ name: "huge.png", mimeType: "image/png", buffer: Buffer.alloc(11 * 1024 * 1024, 1) });
  await upload.getByRole("button", { name: "Upload file" }).click();
  await expect(upload.getByRole("alert")).toHaveText("The file is too large. The limit is 10 MB.");

  const png = Buffer.from("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==", "base64");
  await upload.getByLabel(/Choose a file/).setInputFiles({ name: "referral letter.png", mimeType: "image/png", buffer: png });
  await upload.getByLabel("What is this file?").selectOption("referral");
  await upload.getByRole("button", { name: "Upload file" }).click();
  await expect(upload.getByRole("status")).toHaveText("File uploaded.");
  const row = page.getByTestId("attachment-row").filter({ hasText: "referral_letter.png" });
  await expect(row).toBeVisible();
  await expect(row).toContainText("Referral letter");
  await expect(page.getByRole("link", { name: /^Download/ })).toHaveCount(0);
  await expect(page.getByText("Your role cannot open files.")).toBeVisible();
});

test("clinician: opens a file through the console route (no-store, download)", async ({ page }) => {
  await openFirstParticipant(page, "clinician@example.org");
  await page.getByRole("navigation", { name: "Participant sections" }).getByRole("link", { name: "Files" }).click();
  const link = page.getByRole("link", { name: "Download consent-form-scan.png" });
  await expect(link).toBeVisible();
  const href = (await link.getAttribute("href"))!;
  expect(href).toMatch(/^\/api\/attachments\/[0-9a-f-]+$/);
  const res = await page.request.get(href);
  expect(res.status()).toBe(200);
  expect(res.headers()["cache-control"]).toBe("no-store");
  expect(res.headers()["content-disposition"]).toBe('attachment; filename="consent-form-scan.png"');
  expect((await res.body()).subarray(1, 4).toString()).toBe("PNG");
});

test("clinician: follow-ups are listed earliest due first; marking one done removes it", async ({ page }) => {
  await login(page, "clinician@example.org");
  await expect(page).toHaveURL(/\/[0-9a-f-]{36}\/overview$/);
  await mainNav(page).getByRole("link", { name: "Follow-ups" }).click();
  await expect(page.getByRole("heading", { name: "Follow-ups", level: 1 })).toBeVisible();
  const rows = page.getByTestId("follow-up");
  await expect(rows.first()).toBeVisible();
  const first = rows.first();
  await expect(first).toHaveAttribute("data-overdue", "true");
  await expect(first.getByText("Overdue")).toBeVisible();
  const token = (await first.getByRole("link").innerText()).trim();
  const count = await rows.count();

  await page.getByRole("combobox", { name: /^Show/ }).selectOption("overdue");
  await page.getByRole("button", { name: "Show", exact: true }).click();
  await expect(page).toHaveURL(/show=overdue/);
  for (const v of await rows.evaluateAll((els) => els.map((e) => e.getAttribute("data-overdue")))) expect(v).toBe("true");

  await page.getByRole("combobox", { name: /^Show/ }).selectOption("all");
  await page.getByRole("button", { name: "Show", exact: true }).click();
  await expect(rows).toHaveCount(count);
  await page.getByRole("button", { name: `Mark follow-up for ${token} done` }).first().click();
  await expect(rows).toHaveCount(count - 1);
});

test("search: by code and open follow-up, as a POST; nothing in the address bar", async ({ page }) => {
  await login(page, "clinician@example.org");
  await expect(page).toHaveURL(/\/[0-9a-f-]{36}\/overview$/);
  await mainNav(page).getByRole("link", { name: "Participants" }).click();
  await expect(page).toHaveURL(/\/participants$/);
  const before = page.url();
  const search = page.getByRole("form", { name: "Find a participant" });
  await search.getByLabel("Participant code (start of it)").fill("child-0");
  await search.getByLabel("Hospital record number (MRN)").fill("TU-99999");
  await search.getByRole("button", { name: "Search" }).click();
  // The mock keeps MRNs elsewhere: an MRN filter finds nothing here, and the MRN is cleared from the field.
  await expect(page.getByText("No participant matches.")).toBeVisible();
  await expect(search.getByLabel("Hospital record number (MRN)")).toHaveValue("");
  expect(page.url()).toBe(before);
  await expectNotLeaked(page, "TU-99999");

  await search.getByLabel("Only participants with a follow-up due").check();
  await search.getByRole("button", { name: "Search" }).click();
  const results = page.getByTestId("search-row");
  await expect(results.first()).toBeVisible();
  for (const text of await results.allInnerTexts()) expect(text).toContain("child-0");
  expect(page.url()).toBe(before);
  await results.first().getByRole("link").click();
  await expect(page).toHaveURL(new RegExp("/participants/[0-9a-f-]{36}$"));
});

test("admin: reset a clinician's two-step sign-in with a reason and the step-up", async ({ page }) => {
  await login(page, "admin@example.org");
  await expect(page).toHaveURL(/\/[0-9a-f-]{36}\/overview$/);
  await mainNav(page).getByRole("link", { name: "Users" }).click();
  // Not offered for an owner (higher rank) or for yourself.
  await expect(page.getByRole("button", { name: /^Reset two-step sign-in for owner/ })).toHaveCount(0);
  await expect(page.getByRole("button", { name: "Reset two-step sign-in for admin@demo.example.org" })).toHaveCount(0);
  await page.getByRole("button", { name: "Reset two-step sign-in for clinician4@demo.example.org" }).click();
  const dialog = page.getByRole("dialog");
  await expect(dialog).toContainText("They will have to set up two-step sign-in again");
  await dialog.getByLabel("Why are you resetting it?").fill("Lost phone, confirmed in person");
  await dialog.getByLabel("Your password").fill("wrong-password");
  await dialog.getByRole("button", { name: "Reset two-step sign-in" }).click();
  await expect(dialog.getByRole("alert")).toContainText("was not accepted");
  await expect(dialog.getByLabel("Your password")).toHaveValue("");
  await dialog.getByLabel("Your password").fill("demo");
  await dialog.getByRole("button", { name: "Reset two-step sign-in" }).click();
  await expect(page.getByRole("status").filter({ hasText: "Two-step sign-in was reset for clinician4@demo.example.org" })).toBeVisible();
  await expect(page.getByRole("dialog")).toHaveCount(0);
});

// Last on purpose: a created participant becomes the newest row, which the tests above open as "first participant".
test("admin: create a participant, see its code and QR, find it and open its record", async ({ page }) => {
  await login(page, "admin@example.org");
  await expect(page).toHaveURL(/\/[0-9a-f-]{36}\/overview$/);
  await mainNav(page).getByRole("link", { name: "Participants" }).click();
  await page.getByRole("button", { name: "New participant" }).click();
  const dialog = page.getByRole("dialog", { name: "New participant" });
  await dialog.getByLabel("Age group (optional)").selectOption("24–35 mo");
  await dialog.getByLabel("Language (optional)").selectOption("ne");
  await dialog.getByRole("button", { name: "Create participant" }).click();
  const token = (await page.getByTestId("created-token").innerText()).trim();
  expect(token).toMatch(/^child-[0-9a-f]{20}$/);
  await expect(page.getByRole("img", { name: `QR code for participant ${token}` })).toBeVisible();
  await expectNotLeaked(page, token);
  await page.getByRole("button", { name: "Close" }).click();

  const search = page.getByRole("form", { name: "Find a participant" });
  await search.getByLabel("Participant code (start of it)").fill(token.slice(0, 14));
  await search.getByRole("button", { name: "Search" }).click();
  await expect(page.getByTestId("search-row")).toHaveCount(1);
  await expect(page.getByTestId("search-row")).toContainText("24–35 mo");
  await page.getByTestId("search-row").getByRole("link", { name: token }).click();
  await expect(page).toHaveURL(new RegExp("/participants/[0-9a-f-]{36}$"));
  await expect(page.getByRole("heading", { level: 1 })).toContainText(token);
  await expectNotLeaked(page, token);
  await page.getByRole("navigation", { name: "Participant sections" }).getByRole("link", { name: "Visits" }).click();
  await expect(page.getByText("No visits recorded yet.")).toBeVisible();
});
