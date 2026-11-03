/**
 * Live record-workspace e2e (WP-C.18 against WP-B.19), run by `e2e/live/run-record-live.sh` (`E2E_RECORD=1`; skipped
 * otherwise, so `scripts/live_smoke.sh` is unchanged). A clinician creates a participant (code + QR), the device posts
 * a session for that code, the clinician adds a visit with a summary and a follow-up, links and unlinks the session,
 * clears and sets the follow-up date (PATCH semantics), uploads a PNG and a 10 MiB PDF through the console's route
 * handler and downloads both byte-identical, searches for the participant, and marks the follow-up done. The owner then
 * resets the clinician's two-step sign-in (TRS-3, `204`).
 */
import { createHash } from "node:crypto";
import { expect, test } from "../helpers";
import { RECORD_LIVE, SKIP_REASON, createParticipant, day, env, ingestSession, signIn } from "./support";

test.skip(!RECORD_LIVE, SKIP_REASON);

const PNG = Buffer.from(
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==",
  "base64",
);
const MAX_ATTACHMENT_BYTES = 10_485_760;

/** Exactly the 10 MiB limit: `%PDF-` magic, then deterministic filler (not all zeros, so a truncation shows). */
function bigPdf(): Buffer {
  const buf = Buffer.alloc(MAX_ATTACHMENT_BYTES);
  for (let i = 0; i < buf.length; i++) buf[i] = (i * 31 + (i >> 9)) & 0xff;
  buf.write("%PDF-1.4\n", 0, "latin1");
  return buf;
}

const sha = (b: Buffer) => createHash("sha256").update(b).digest("hex");

test("clinician: participant + QR -> visit, session link, follow-up -> files round trip -> search -> follow-up done", async ({
  page,
}) => {
  test.setTimeout(600_000);
  const org = env("E2E_ORG_ID");
  await signIn(page, env("E2E_EMAIL"), org);
  const { token, path } = await createParticipant(page, org);
  const sessionId = await ingestSession(page.request, token);

  // ---- visit with a summary and a follow-up ----
  const tabs = page.getByRole("navigation", { name: "Participant sections" });
  await tabs.getByRole("link", { name: "Visits" }).click();
  await page.getByRole("button", { name: "Add a visit" }).click();
  await page.getByLabel("Reason for the visit (short)").fill("Live smoke visit");
  await page.getByLabel("Visit summary").fill("Live smoke summary text");
  await page.getByLabel("Follow up by (optional)").fill(day(1));
  await page.getByRole("button", { name: "Save visit" }).click();
  const visit = page.getByTestId("visit").filter({ hasText: "Live smoke visit" });
  await expect(visit.getByTestId("visit-summary")).toHaveText("Live smoke summary text");
  await expect(visit).toContainText("Follow up by");

  // ---- link the device's session, unlink it, link it again (POST / DELETE /encounters/{id}/sessions) ----
  await visit.getByLabel("Link a session").selectOption({ index: 1 });
  await visit.getByRole("button", { name: "Link", exact: true }).click();
  const sessionLink = visit.locator(`a[href="/${org}/sessions/${encodeURIComponent(sessionId)}"]`);
  await expect(sessionLink).toBeVisible();
  await visit.getByRole("button", { name: /^Remove session / }).click();
  await expect(sessionLink).toHaveCount(0);
  await expect(visit).toContainText("No sessions linked.");
  await visit.getByLabel("Link a session").selectOption({ index: 1 });
  await visit.getByRole("button", { name: "Link", exact: true }).click();
  await expect(sessionLink).toBeVisible();

  // ---- PATCH: clearing the follow-up date clears it (the backend clears on "", keeps on null) ----
  await visit.getByRole("button", { name: "Change" }).click();
  const edit = page.getByRole("form", { name: "Change visit" });
  await edit.getByLabel("Follow up by (optional)").fill("");
  await edit.getByRole("button", { name: "Save visit" }).click();
  await expect(edit).toHaveCount(0);
  await expect(visit).not.toContainText("Follow up by");
  await expect(visit.getByTestId("visit-summary")).toHaveText("Live smoke summary text");
  await visit.getByRole("button", { name: "Change" }).click();
  await edit.getByLabel("Follow up by (optional)").fill(day(1));
  await edit.getByRole("button", { name: "Save visit" }).click();
  await expect(edit).toHaveCount(0);
  await expect(visit).toContainText("Follow up by");

  // ---- files: a PNG and an exactly-10-MiB PDF through /api/attachments/{id}/content, back byte-identical ----
  await tabs.getByRole("link", { name: "Files" }).click();
  const upload = page.getByRole("form", { name: "Add a file" });
  await upload.getByLabel(/Choose a file/).setInputFiles({ name: "smoke.png", mimeType: "image/png", buffer: PNG });
  await upload.getByRole("button", { name: "Upload file" }).click();
  await expect(upload.getByRole("status")).toHaveText("File uploaded.");
  const pngLink = page.getByRole("link", { name: "Download smoke.png" });
  await expect(pngLink).toBeVisible();
  const png = await page.request.get((await pngLink.getAttribute("href"))!);
  expect(png.status()).toBe(200);
  expect(png.headers()["cache-control"]).toBe("no-store");
  expect(png.headers()["content-type"]).toBe("image/png");
  expect(png.headers()["content-disposition"]).toBe('attachment; filename="smoke.png"');
  expect(Buffer.compare(await png.body(), PNG)).toBe(0);
  // The list is the real `GET /participants/{id}/attachments`: the row's link is the attachment id.
  expect(await pngLink.getAttribute("href")).toMatch(/^\/api\/attachments\/[0-9a-f-]{36}$/);

  const pdf = bigPdf();
  await upload.getByLabel(/Choose a file/).setInputFiles({ name: "scan form.pdf", mimeType: "application/pdf", buffer: pdf });
  await upload.getByRole("button", { name: "Upload file" }).click();
  await expect(upload.getByRole("status")).toHaveText("File uploaded.", { timeout: 120_000 });
  // The server sanitises the name (`emr::sanitize_file_name`).
  const pdfLink = page.getByRole("link", { name: "Download scan_form.pdf" });
  await expect(pdfLink).toBeVisible();
  const back = await page.request.get((await pdfLink.getAttribute("href"))!, { timeout: 120_000 });
  expect(back.status()).toBe(200);
  expect(back.headers()["content-disposition"]).toBe('attachment; filename="scan_form.pdf"');
  const body = await back.body();
  expect(body.length).toBe(MAX_ATTACHMENT_BYTES);
  expect(sha(body)).toBe(sha(pdf));

  // One byte over the limit is refused by the route handler before it reaches the backend.
  const over = await page.request.put("/api/attachments/00000000-0000-4000-8000-000000000000/content", {
    headers: { "Content-Type": "application/pdf" },
    data: Buffer.concat([pdf, Buffer.from([0])]),
    timeout: 120_000,
  });
  expect(over.status()).toBe(400);
  expect((await over.json()).slug).toBe("attachment-too-large");

  // ---- search finds the participant by code prefix (POST body, never the URL) ----
  await page.goto(`/${org}/participants`);
  await page.getByLabel("Participant code (start of it)").fill(token.slice(0, 14));
  await page.getByLabel("Only participants with a follow-up due").check();
  await page.getByRole("button", { name: "Search", exact: true }).click();
  const hit = page.getByTestId("search-row").filter({ has: page.locator(`a[href="${path}"]`) });
  await expect(hit).toBeVisible();
  // Row counters (REC-2): one session, one open visit, the next follow-up date.
  await expect(hit.locator('td[data-label="Sessions"]')).toHaveText("1");
  await expect(hit.locator('td[data-label="Open visits"]')).toHaveText("1");
  await expect(hit.locator('td[data-label="Next follow-up"]')).not.toHaveText("-");
  expect(page.url()).not.toContain(token.slice(0, 14));

  // ---- follow-ups: due within the week (due_before), mark done ----
  await page.goto(`/${org}/follow-ups?show=week`);
  const row = page.getByTestId("follow-up").filter({ hasText: token });
  await expect(row).toBeVisible();
  await row.getByRole("button", { name: `Mark follow-up for ${token} done` }).click();
  await expect(row).toHaveCount(0);
  await page.goto(`${path}?tab=visits`);
  await expect(page.getByTestId("visit").filter({ hasText: "Live smoke visit" })).toContainText("Follow-up done");
});

test("owner resets the clinician's two-step sign-in (TRS-3)", async ({ page }) => {
  const org = env("E2E_ORG_ID");
  const clinician = env("E2E_EMAIL");
  await signIn(page, env("E2E_OWNER_EMAIL"), org);
  await page.goto(`/${org}/users`);
  await page.getByRole("button", { name: `Reset two-step sign-in for ${clinician}` }).click();
  const dialog = page.getByRole("dialog");
  await dialog.getByLabel("Why are you resetting it?").fill("Lost phone, confirmed in person");
  await dialog.getByLabel("Your password").fill("not-the-password-123");
  await dialog.getByRole("button", { name: "Reset two-step sign-in" }).click();
  await expect(dialog.getByRole("alert")).toContainText("was not accepted");
  await expect(dialog.getByLabel("Your password")).toHaveValue("");
  await dialog.getByLabel("Your password").fill(env("E2E_PASSWORD"));
  await dialog.getByRole("button", { name: "Reset two-step sign-in" }).click();
  await expect(page.getByRole("status").filter({ hasText: `Two-step sign-in was reset for ${clinician}` })).toBeVisible();
});
