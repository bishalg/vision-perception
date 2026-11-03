/**
 * WP-C.16 CSP-1..4 against the production build (`prod` project only: `next build` + `next start`, mock mode).
 * `next dev` adds `'unsafe-eval'` and never serves prerendered HTML, so only this run shows that the nonce policy
 * holds for real: every page below must load with no `securitypolicyviolation` (the `test` from helpers fails on any),
 * carry the strict policy, stamp the request's nonce on every script it serves, and hydrate.
 */
import type { Page, Response } from "@playwright/test";
import { expect, test, login, loginToOverview, MOCK_ORG_ID } from "./helpers";

const HSTS = "max-age=63072000; includeSubDomains";
const nonceOf = (csp: string) => /'nonce-([^']+)'/.exec(csp)?.[1];

/** The served policy is the production one, and every script tag in the raw HTML carries this response's nonce. */
async function expectStrictDocument(response: Response | null): Promise<void> {
  expect(response, "document response").not.toBeNull();
  const headers = response!.headers();
  const csp = headers["content-security-policy"] ?? "";
  const scriptSrc = /(?:^|;\s*)script-src ([^;]*)/.exec(csp)?.[1] ?? "";
  expect(scriptSrc).toContain("'strict-dynamic'");
  expect(scriptSrc).not.toContain("'unsafe-eval'");
  expect(scriptSrc).not.toContain("'unsafe-inline'");
  expect(headers["strict-transport-security"]).toBe(HSTS);
  const nonce = nonceOf(csp);
  expect(nonce).toBeTruthy();
  const html = await response!.text();
  const scripts = [...html.matchAll(/<script\b[^>]*>/gi)].map((m) => m[0]);
  expect(scripts.length, "server-rendered scripts").toBeGreaterThan(0);
  for (const tag of scripts) expect(tag, "every script carries the nonce").toContain(`nonce="${nonce}"`);
}

/** React attached to the server HTML (the bundles ran under the policy and hydrated the page). */
async function expectHydrated(page: Page, selector = "h1"): Promise<void> {
  await page.waitForFunction(
    (sel) => {
      const el = document.querySelector(sel);
      return Boolean(el && Object.keys(el).some((k) => k.startsWith("__reactFiber$")));
    },
    selector,
    { timeout: 15_000 },
  );
}

test.beforeEach(() => {
  test.skip(test.info().project.name !== "prod", "production-build CSP checks");
});

test("login page: strict policy, nonced scripts, hydrated", async ({ page }) => {
  await expectStrictDocument(await page.goto("/login"));
  await expectHydrated(page);
});

test("proxy redirects carry the policy and HSTS", async ({ request }) => {
  const res = await request.get("/some-org/overview", { maxRedirects: 0 });
  expect(res.status()).toBe(307);
  expect(res.headers()["location"]).toContain("/login?next=");
  expect(res.headers()["content-security-policy"]).toMatch(/script-src 'self' 'nonce-[^']+' 'strict-dynamic'/);
  expect(res.headers()["strict-transport-security"]).toBe(HSTS);
});

test("org overview after sign-in", async ({ page }) => {
  const org = await loginToOverview(page);
  await expectStrictDocument(await page.goto(`/${org}/overview`));
  await expectHydrated(page);
});

test("devices: enrol-code QR renders under the policy", async ({ page }) => {
  const org = await loginToOverview(page);
  await expectStrictDocument(await page.goto(`/${org}/devices`));
  await page.getByRole("button", { name: "Generate enrol code" }).click();
  const code = page.getByTestId("enrol-code");
  await expect(code).toHaveText(/\S{4,}/);
  const text = (await code.innerText()).trim();
  await expect(page.getByRole("img", { name: `QR code for enrol code ${text}` })).toBeVisible();
});

test("participant print view", async ({ page }) => {
  test.setTimeout(180_000);
  await login(page, "clinician@example.org");
  await expect(page).toHaveURL(/\/[0-9a-f-]{36}\/overview$/);
  const org = new URL(page.url()).pathname.split("/")[1]!;
  await page.goto(`/${org}/participants`);
  await page.locator("table.data tbody tr").first().getByRole("link").first().click();
  await expect(page).toHaveURL(new RegExp(`/${org}/participants/[0-9a-f-]{36}$`), { timeout: 90_000 });
  await page.getByRole("link", { name: "Print summary" }).click();
  await expect(page.getByRole("button", { name: "Print" })).toBeVisible();
  // Load the print view as a document too, so its raw HTML is checked.
  await expectStrictDocument(await page.goto(page.url()));
  await page.emulateMedia({ media: "print" });
  await expect(page.getByRole("heading", { name: "Participant clinical summary" })).toBeVisible();
  await page.emulateMedia({ media: "screen" });
});

test("scoring-profile editor loads asd_wasm under the policy", async ({ page }) => {
  const wasmLoaded = page.waitForResponse((r) => r.url().includes("/asd-wasm/asd_wasm_bg.wasm"));
  await login(page, "superadmin@example.org");
  await page.getByLabel("Organisation id").fill(MOCK_ORG_ID);
  await page.getByRole("button", { name: "Switch" }).click();
  await page.getByRole("navigation", { name: "Main navigation" }).getByRole("link", { name: "Scoring", exact: true }).click();
  await page.getByRole("link", { name: /New draft from / }).click();
  await expect(page.getByText(/core unavailable/i)).toHaveCount(0);
  expect((await wasmLoaded).ok()).toBe(true);
  await expectStrictDocument(await page.goto(page.url()));
});

test("research workbench: registry form, preview cards and SVG charts load under the policy", async ({ page }) => {
  const wasmLoaded = page.waitForResponse((r) => r.url().includes("/asd-wasm/asd_wasm_bg.wasm"));
  await login(page, "researcher@example.org");
  await expect(page).toHaveURL(/\/[0-9a-f-]{36}\/overview$/);
  const org = new URL(page.url()).pathname.split("/")[1]!;
  await page.goto(`/${org}/research/experiments`);
  await page.getByRole("link", { name: "Lower I-VT threshold" }).click();
  await expect(page.getByRole("heading", { name: "Lower I-VT threshold" })).toBeVisible();
  await expectStrictDocument(await page.goto(page.url()));
  await expectHydrated(page);
  await expect(page.getByRole("article")).toHaveCount(2, { timeout: 30_000 });
  expect((await wasmLoaded).ok()).toBe(true);
  await page.getByRole("combobox", { name: /^Parameter/ }).selectOption("flag.greenFloor");
  await page.getByRole("button", { name: "Run sweep" }).click();
  await expect(page.getByTestId("sweep-curve")).toBeVisible();
});

test("research runs: the 625-cell sweep heatmaps and the age tables render under the policy", async ({ page }) => {
  await login(page, "researcher@example.org");
  await expect(page).toHaveURL(/\/[0-9a-f-]{36}\/overview$/);
  const org = new URL(page.url()).pathname.split("/")[1]!;
  // Seeded in mock/research.ts (WP-X.9): a two-axis param_sweep run and an age_profile run.
  await expectStrictDocument(await page.goto(`/${org}/research/runs/a0000000-0000-4000-8000-000000000004`));
  await expectHydrated(page);
  await expect(page.getByTestId("sweep-cell")).toHaveCount(1250);
  await expectStrictDocument(await page.goto(`/${org}/research/runs/a0000000-0000-4000-8000-000000000005`));
  await expectHydrated(page);
  await expect(page.getByTestId("age-row").first()).toBeVisible();
});

test("unmatched URL: 404 page is rendered per request, nonced and hydrated", async ({ page }) => {
  const org = await loginToOverview(page);
  for (const path of [`/${org}/no-such-page`, `/${org}/sessions/x/y/z`]) {
    const res = await page.goto(path);
    expect(res!.status()).toBe(404);
    await expectStrictDocument(res);
    await expect(page.getByRole("heading", { name: "Not found" })).toBeVisible();
    await expectHydrated(page);
  }
});
