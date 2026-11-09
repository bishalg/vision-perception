/**
 * Live share-a-case e2e (WP-C.19 against WP-I.13), run by `e2e/live/run-record-live.sh` (`E2E_RECORD=1`). A clinician
 * creates a participant, the device posts a session for it, the clinician shares the case de-identified from the
 * record, waits for the worker to build the ZIP, and creates a share link (passcode shown once). An anonymous browser
 * opens the public page, is refused with a wrong passcode, opens it with the right one (typed in lower case with
 * dashes), downloads the ZIP (byte-identical to the export's recorded sha256), and is refused a second download
 * (`max_downloads` 1) both at the passcode step and with the spent download token. A second link is revoked and then
 * answers like an unknown one.
 */
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import type { Browser, Page } from "@playwright/test";
import { expect, test, paths } from "../helpers";
import { RECORD_LIVE, SKIP_REASON, apiToken, createParticipant, env, ingestSession, signIn } from "./support";

test.skip(!RECORD_LIVE, SKIP_REASON);

/** On the export page: create a link, read the URL and the passcode shown once, close the box. */
async function createLink(page: Page): Promise<{ url: string; path: string; id: string; passcode: string }> {
  const panel = page.getByRole("region", { name: "Share links" });
  await panel.getByRole("button", { name: "Create share link" }).click();
  const box = panel.getByTestId("share-created");
  await expect(box).toBeVisible();
  const url = (await box.getByTestId("share-url").textContent())!.trim();
  const passcode = (await box.getByTestId("share-passcode").textContent())!.trim();
  expect(passcode).toMatch(/^[A-HJ-NP-Z2-9]{12}$/);
  await box.getByRole("button", { name: /close/ }).click();
  await expect(panel.getByTestId("share-created")).toHaveCount(0);
  await expect(page.locator("body")).not.toContainText(passcode);
  const path = new URL(url).pathname;
  return { url, path, id: path.split("/").pop()!, passcode };
}

async function enterPasscode(page: Page, passcode: string) {
  await page.getByLabel("Passcode").fill(passcode);
  await page.getByRole("button", { name: "Continue" }).click();
}

async function anonymous(browser: Browser) {
  const context = await browser.newContext();
  return { context, page: await context.newPage() };
}

test("clinician shares a case de-identified; the recipient downloads the ZIP once with the passcode", async ({
  page,
  browser,
}) => {
  test.setTimeout(600_000);
  const org = env("E2E_ORG_ID");
  await signIn(page, env("E2E_EMAIL"), org);
  const { token, path: record } = await createParticipant(page, org);
  await ingestSession(page.request, token);

  // ---- share the case from the record (de-identified: no consent fields) ----
  await page.goto(record);
  await page.getByRole("button", { name: "Share case" }).click();
  const dialog = page.getByRole("dialog", { name: "Share this case" });
  await expect(dialog.getByRole("radio", { name: "De-identified" })).toBeChecked();
  await dialog.getByLabel(/^Recipient/).fill("Dr Example, Example Children's Hospital");
  await dialog.getByLabel(/^Reason/).fill("Second opinion requested by the family");
  await dialog.getByRole("button", { name: "Create case package" }).click();
  await expect(page.getByRole("dialog")).toHaveCount(0);
  await page.getByRole("link", { name: "Open the export to share it" }).click();
  await expect(page).toHaveURL(new RegExp(`${paths(org).data.exports}/[0-9a-f-]{36}$`));
  const exportId = new URL(page.url()).pathname.split("/").pop()!;

  // The worker builds the ZIP; the page offers links once the export has succeeded.
  const panel = page.getByRole("region", { name: "Share links" });
  await expect(async () => {
    await page.reload();
    await expect(panel.getByRole("button", { name: "Create share link" })).toBeEnabled({ timeout: 5_000 });
  }).toPass({ timeout: 180_000, intervals: [2_000] });

  // `Export.case_participant_id` (I.13) points back at the record.
  const bearer = await apiToken(page.request, env("E2E_EMAIL"));
  const exp = await (
    await page.request.get(`${env("E2E_API_URL")}/v1/exports/${exportId}`, { headers: { Authorization: `Bearer ${bearer}` } })
  ).json();
  expect(exp.format).toBe("case_package");
  expect(exp.mode).toBe("deidentified");
  expect(record.endsWith(`/${exp.case_participant_id}`)).toBe(true);

  const link = await createLink(page);
  // `ShareLink.url` is absolute: the backend knows the console origin (ASD_CONSOLE_ORIGIN).
  expect(link.url).toBe(`${env("E2E_BASE_URL")}/share/${link.id}`);
  const row = page.locator(`tr[data-link-id="${link.id}"]`);
  await expect(row).toHaveAttribute("data-state", "active");

  // ---- the recipient, in a browser with no session ----
  const { context, page: recipient } = await anonymous(browser);
  const response = await recipient.goto(link.path);
  expect(response!.status()).toBe(200);
  expect(response!.headers()["cache-control"]).toContain("no-store");
  expect(response!.headers()["x-robots-tag"]).toBe("noindex, nofollow");
  await expect(recipient.getByRole("heading", { name: "Download a shared file" })).toBeVisible();

  await enterPasscode(recipient, "WRONGPASS234");
  await expect(recipient.locator(".error[role='alert']")).toContainText("passcode is not correct");
  // Typing aids: lower case and dashes are normalised before the backend sees the passcode.
  const typed = `${link.passcode.slice(0, 4)}-${link.passcode.slice(4, 8)}-${link.passcode.slice(8)}`.toLowerCase();
  await enterPasscode(recipient, typed);
  await expect(recipient.getByTestId("share-downloads-left")).toHaveText("1");
  await expect(recipient.getByTestId("share-file")).toHaveText(`export-${exportId}-case-package.zip`);
  const spentToken = await recipient.locator('input[name="t"]').inputValue();
  const [download] = await Promise.all([
    recipient.waitForEvent("download"),
    recipient.getByRole("button", { name: "Download" }).click(),
  ]);
  expect(download.suggestedFilename()).toBe(`export-${exportId}-case-package.zip`);
  const zip = readFileSync((await download.path())!);
  expect(zip.subarray(0, 2).toString("latin1")).toBe("PK");
  expect(zip.length).toBe(exp.byte_size ?? zip.length);
  const done = await (
    await page.request.get(`${env("E2E_API_URL")}/v1/exports/${exportId}`, { headers: { Authorization: `Bearer ${bearer}` } })
  ).json();
  expect(createHash("sha256").update(zip).digest("hex")).toBe(done.sha256);
  expect(zip.includes(Buffer.from("manifest.json"))).toBe(true);
  await expect(recipient.getByRole("status")).toContainText("download has started");
  expect(recipient.url()).not.toContain(link.passcode);

  // ---- max one download: the passcode step and the spent token are both refused (410 share-link-exhausted) ----
  await enterPasscode(recipient, link.passcode);
  await expect(recipient.locator(".error[role='alert']")).toContainText("download limit");
  const replay = await recipient.request.post(`/api/share/${link.id}`, { form: { t: spentToken }, maxRedirects: 0 });
  expect(replay.status()).toBe(303);
  expect(replay.headers()["location"]).toBe(`/share/${link.id}?e=share-link-exhausted`);
  expect(await context.cookies()).toEqual([]);
  await context.close();

  await page.reload();
  await expect(row).toHaveAttribute("data-state", "exhausted");

  // ---- a revoked link answers like an unknown one (DELETE /share-links/{id} -> 404 at open) ----
  const second = await createLink(page);
  const secondRow = page.locator(`tr[data-link-id="${second.id}"]`);
  await secondRow.getByRole("button", { name: "Revoke" }).click();
  await secondRow.getByRole("button", { name: "Yes, revoke" }).click();
  await expect(secondRow).toHaveAttribute("data-state", "revoked");
  const other = await anonymous(browser);
  await other.page.goto(second.path);
  await enterPasscode(other.page, second.passcode);
  await expect(other.page.locator(".error[role='alert']")).toContainText("does not exist or has been withdrawn");
  await other.context.close();
});
