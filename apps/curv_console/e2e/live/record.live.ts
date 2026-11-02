/**
 * Live record-workspace smoke (WP-C.18 against WP-B.19), for the wave 2 integration: run with the live smoke config
 * and `E2E_RECORD=1` once the B.19 routes are merged. A clinician creates a participant (code + QR), adds a visit with
 * a summary and a follow-up, uploads a PNG with `Content-Digest` through the console route, downloads the same bytes
 * back, finds the follow-up and marks it done. Skipped unless `E2E_RECORD=1`, so `scripts/live_smoke.sh` is unchanged.
 */
import { expect, test } from "../helpers";

const env = (name: string): string => {
  const v = process.env[name];
  if (!v) throw new Error(`${name} is required (set by scripts/live_smoke.sh)`);
  return v;
};

test.skip(!process.env.E2E_RECORD, "set E2E_RECORD=1 to run the record workspace against a backend with WP-B.19");

const PNG = Buffer.from(
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==",
  "base64",
);

test("clinician: create participant -> visit with follow-up -> file round trip -> follow-up done", async ({ page }) => {
  const org = env("E2E_ORG_ID");
  await page.goto("/login");
  await page.getByLabel("Email").fill(env("E2E_EMAIL"));
  await page.getByLabel("Password").fill(env("E2E_PASSWORD"));
  await page.getByRole("button", { name: "Sign in" }).click();
  await expect(page).toHaveURL(new RegExp(`/${org}/overview$`), { timeout: 120_000 });

  await page.goto(`/${org}/participants`);
  await page.getByRole("button", { name: "New participant" }).click();
  await page.getByRole("button", { name: "Create participant" }).click();
  const token = (await page.getByTestId("created-token").innerText()).trim();
  expect(token).toMatch(/^child-[0-9a-f]{20}$/);
  await expect(page.getByRole("img", { name: `QR code for participant ${token}` })).toBeVisible();
  await page.getByRole("link", { name: "Open record" }).click();
  await expect(page).toHaveURL(new RegExp(`/${org}/participants/[0-9a-f-]{36}$`));

  const tabs = page.getByRole("navigation", { name: "Participant sections" });
  await tabs.getByRole("link", { name: "Visits" }).click();
  await page.getByRole("button", { name: "Add a visit" }).click();
  await page.getByLabel("Reason for the visit (short)").fill("Live smoke visit");
  await page.getByLabel("Visit summary").fill("Live smoke summary text");
  const tomorrow = new Date(Date.now() + 86_400_000);
  const day = `${tomorrow.getFullYear()}-${String(tomorrow.getMonth() + 1).padStart(2, "0")}-${String(tomorrow.getDate()).padStart(2, "0")}`;
  await page.getByLabel("Follow up by (optional)").fill(day);
  await page.getByRole("button", { name: "Save visit" }).click();
  await expect(page.getByTestId("visit").filter({ hasText: "Live smoke visit" }).getByTestId("visit-summary")).toHaveText(
    "Live smoke summary text",
  );

  await tabs.getByRole("link", { name: "Files" }).click();
  const upload = page.getByRole("form", { name: "Add a file" });
  await upload.getByLabel(/Choose a file/).setInputFiles({ name: "smoke.png", mimeType: "image/png", buffer: PNG });
  await upload.getByRole("button", { name: "Upload file" }).click();
  await expect(upload.getByRole("status")).toHaveText("File uploaded.");
  const link = page.getByRole("link", { name: "Download smoke.png" });
  await expect(link).toBeVisible();
  const res = await page.request.get((await link.getAttribute("href"))!);
  expect(res.status()).toBe(200);
  expect(res.headers()["cache-control"]).toBe("no-store");
  expect(Buffer.compare(await res.body(), PNG)).toBe(0);

  await page.goto(`/${org}/follow-ups`);
  const row = page.getByTestId("follow-up").filter({ hasText: token });
  await expect(row).toBeVisible();
  await row.getByRole("button", { name: `Mark follow-up for ${token} done` }).click();
  await expect(row).toHaveCount(0);
});
