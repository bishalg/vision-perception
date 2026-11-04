import { type Browser, type Page } from "@playwright/test";
import { expect, test, LIVE, login } from "./helpers";

// WP-C.19 (SHR-8..10, HIP-9). Mock mode: synthetic seeded case package and share links, one per state.
test.skip(LIVE, "share flows use the mock backend's seeded case package and links");
test.describe.configure({ timeout: 240_000 });

/** Mirrors `MOCK_CASE_EXPORT_ID`, `MOCK_SHARE_LINKS` and `MOCK_SHARE_PASSCODE` of src/lib/api/mock (synthetic). */
const CASE_EXPORT = "e20c0de0-0000-4000-8000-000000000005";
const LINKS = {
  active: "5a5e0000-0000-4000-8000-000000000001",
  expired: "5a5e0000-0000-4000-8000-000000000002",
  exhausted: "5a5e0000-0000-4000-8000-000000000003",
  revoked: "5a5e0000-0000-4000-8000-000000000004",
};
const PASSCODE = "MOCKPASS2345";

async function signIn(page: Page, email: string): Promise<string> {
  await login(page, email);
  await expect(page).toHaveURL(/\/[0-9a-f-]{36}\/overview$/);
  return new URL(page.url()).pathname.split("/")[1]!;
}

/** Create a share link on the export page that is open; returns the link path and the passcode shown once. */
async function createLink(page: Page): Promise<{ path: string; passcode: string }> {
  const panel = page.getByRole("region", { name: "Share links" });
  await panel.getByRole("button", { name: "Create share link" }).click();
  const box = panel.getByTestId("share-created");
  await expect(box).toBeVisible();
  await expect(box).toContainText("another channel");
  const url = (await box.getByTestId("share-url").textContent())!.trim();
  const passcode = (await box.getByTestId("share-passcode").textContent())!.trim();
  expect(passcode).toMatch(/^[A-HJ-NP-Z2-9]{12}$/);
  await box.getByRole("button", { name: /close/ }).click();
  await expect(panel.getByTestId("share-created")).toHaveCount(0);
  // Shown once: the passcode is nowhere on the page any more, and not in the URL.
  await expect(page.locator("body")).not.toContainText(passcode);
  expect(page.url()).not.toContain(passcode);
  return { path: new URL(url).pathname, passcode };
}

async function anonymousPage(browser: Browser) {
  const context = await browser.newContext();
  return { context, page: await context.newPage() };
}

async function enterPasscode(page: Page, passcode: string) {
  await page.getByLabel("Passcode").fill(passcode);
  await page.getByRole("button", { name: "Continue" }).click();
}

test("clinician shares a case; the recipient downloads it with the passcode once", async ({ page, browser }) => {
  const org = await signIn(page, "clinician@example.org");
  await page.goto(`/${org}/participants`);
  await page.locator("table.data tbody tr").first().getByRole("link").first().click();
  await expect(page).toHaveURL(new RegExp(`/${org}/participants/[0-9a-f-]{36}$`), { timeout: 90_000 });

  await page.getByRole("button", { name: "Share case" }).click();
  const dialog = page.getByRole("dialog", { name: "Share this case" });
  await dialog.getByRole("radio", { name: "Identified", exact: true }).check();
  await dialog.getByLabel(/^Recipient/).fill("Dr Example, Example Children's Hospital, Boston");
  await dialog.getByLabel(/^Reason/).fill("Second opinion requested by the family");
  await dialog.getByRole("button", { name: "Create case package" }).click();
  // Refused before any request: identified needs the consent reference and date.
  await expect(dialog.getByRole("alert")).toContainText("consent reference");
  await dialog.getByLabel(/^Consent reference/).fill("CF-2026-114");
  await dialog.getByLabel("Date consent was recorded").fill("2026-10-30");
  await dialog.getByRole("button", { name: "Create case package" }).click();
  await expect(page.getByRole("dialog")).toHaveCount(0);
  await page.getByRole("link", { name: "Open the export to share it" }).click();
  await expect(page).toHaveURL(new RegExp(`/${org}/exports/[0-9a-f-]{36}$`));
  await expect(page.getByText("Case package (ZIP)").first()).toBeVisible();

  const { path, passcode } = await createLink(page);
  const row = page.locator(`tr[data-link-id="${path.split("/").pop()}"]`);
  await expect(row).toHaveAttribute("data-state", "active");

  const { context, page: recipient } = await anonymousPage(browser);
  const response = await recipient.goto(path);
  expect(response!.headers()["cache-control"]).toContain("no-store");
  expect(response!.headers()["x-robots-tag"]).toBe("noindex, nofollow");
  expect(response!.headers()["content-security-policy"]).toMatch(/'nonce-[^']+'/);
  await expect(recipient.getByRole("heading", { name: "Download a shared file" })).toBeVisible();
  await expect(recipient.locator('meta[name="robots"]')).toHaveAttribute("content", /noindex/);

  await enterPasscode(recipient, "WRONGPASS234");
  await expect(recipient.locator(".error[role='alert']")).toContainText("passcode is not correct");
  await enterPasscode(recipient, passcode.toLowerCase());
  await expect(recipient.getByTestId("share-downloads-left")).toHaveText("1");
  const [download] = await Promise.all([
    recipient.waitForEvent("download"),
    recipient.getByRole("button", { name: "Download" }).click(),
  ]);
  expect(download.suggestedFilename()).toMatch(/^case-package-.*\.zip$/);
  await expect(recipient.getByRole("status")).toContainText("download has started");
  expect(recipient.url()).not.toContain(passcode);

  // One download allowed by default: the next open says so.
  await enterPasscode(recipient, passcode);
  await expect(recipient.locator(".error[role='alert']")).toContainText("download limit");
  // The public page and route set no cookie.
  expect(await context.cookies()).toEqual([]);
  await context.close();

  await page.reload();
  await expect(row).toHaveAttribute("data-state", "exhausted");
});

test("a revoked link answers like an unknown one", async ({ page, browser }) => {
  const org = await signIn(page, "owner@example.org");
  await page.goto(`/${org}/exports/${CASE_EXPORT}`);
  const { path, passcode } = await createLink(page);
  const row = page.locator(`tr[data-link-id="${path.split("/").pop()}"]`);
  await row.getByRole("button", { name: "Revoke" }).click();
  await row.getByRole("button", { name: "Yes, revoke" }).click();
  await expect(row).toHaveAttribute("data-state", "revoked");

  const { context, page: recipient } = await anonymousPage(browser);
  await recipient.goto(path);
  await enterPasscode(recipient, passcode);
  await expect(recipient.locator(".error[role='alert']")).toContainText("does not exist or has been withdrawn");
  await recipient.goto("/share/00000000-0000-4000-8000-000000000000");
  await enterPasscode(recipient, passcode);
  await expect(recipient.locator(".error[role='alert']")).toContainText("does not exist or has been withdrawn");
  await context.close();
});

test("expired, used-up and revoked seeded links show their own message", async ({ page }) => {
  for (const [id, text] of [
    [LINKS.expired, "has expired"],
    [LINKS.exhausted, "download limit"],
    [LINKS.revoked, "does not exist or has been withdrawn"],
  ] as const) {
    await page.goto(`/share/${id}`);
    await enterPasscode(page, PASSCODE);
    await expect(page.locator(".error[role='alert']")).toContainText(text);
  }
  // The download route's refusal comes back as an error key only.
  await page.goto(`/share/${LINKS.active}?e=token`);
  await expect(page.locator(".error[role='alert']")).toContainText("Enter the passcode again");
  await page.goto(`/share/${LINKS.active}?e=toString`);
  await expect(page.locator(".error[role='alert']")).toHaveCount(0);
});

test("a signed-in user opening a share link stays on it", async ({ page, context }) => {
  await signIn(page, "clinician@example.org");
  const before = (await context.cookies()).map((c) => `${c.name}=${c.value}`).sort();
  const response = await page.goto(`/share/${LINKS.active}`);
  expect(response!.status()).toBe(200);
  await expect(page).toHaveURL(new RegExp(`/share/${LINKS.active}$`));
  await expect(page.getByRole("heading", { name: "Download a shared file" })).toBeVisible();
  await enterPasscode(page, PASSCODE);
  await expect(page.getByRole("button", { name: "Download" })).toBeVisible();
  // No session or HIP-2 cookie was refreshed or cleared by the public page.
  const after = (await context.cookies()).map((c) => `${c.name}=${c.value}`).sort();
  expect(after).toEqual(before);
});

test("the public share page follows Accept-Language and keeps the English mandatory sentence", async ({ browser }) => {
  const ne = await browser.newContext({ locale: "ne-NP", extraHTTPHeaders: { "Accept-Language": "ne-NP,en;q=0.5" } });
  const page = await ne.newPage();
  await page.goto(`/share/${LINKS.active}`);
  await expect(page.locator("html")).toHaveAttribute("lang", "ne-NP");
  await expect(page.getByRole("heading", { name: "साझा गरिएको फाइल डाउनलोड गर्नुहोस्", level: 1 })).toBeVisible();
  await expect(page.getByText("This is not a diagnostic tool.")).toBeVisible();
  await page.getByLabel("पासकोड").fill("WRONGPASS234");
  await page.getByRole("button", { name: "जारी राख्नुहोस्" }).click();
  await expect(page.locator(".error[role='alert']")).toContainText("पासकोड मिलेन");
  await ne.close();
});
