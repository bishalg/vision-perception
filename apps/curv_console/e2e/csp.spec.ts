/**
 * WP-C.16 CSP-1..4 (mock mode and live): headers on HTML responses, and the pages that need scripts, wasm, blob
 * images or print styles raise no `securitypolicyviolation` (the `test` from helpers fails on any event). The
 * enrol-code QR (devices.spec.ts) and the participant print view (participant.spec.ts) are covered by those specs
 * running under the same listener, and again against the production build by csp-prod.spec.ts.
 *
 * Runs in both projects: `chromium` (next dev: `'unsafe-eval'` allowed, no HSTS) and `prod` (next start: neither).
 */
import { expect, test, LIVE, MOCK_ORG_ID, login } from "./helpers";

const nonceOf = (csp: string) => /'nonce-([^']+)'/.exec(csp)?.[1];

test("every HTML response carries a per-request nonce CSP, and HSTS in production", async ({ page, request }) => {
  const production = test.info().project.name === "prod";
  const first = await page.goto("/login");
  const csp = first!.headers()["content-security-policy"]!;
  expect(csp).toContain("script-src 'self' 'nonce-");
  expect(csp).toContain("'strict-dynamic' 'wasm-unsafe-eval'");
  expect(csp).toContain("frame-ancestors 'none'");
  expect(csp).not.toMatch(/script-src[^;]*'unsafe-inline'/);
  if (production) {
    expect(csp).not.toMatch(/script-src[^;]*'unsafe-eval'/);
    expect(first!.headers()["strict-transport-security"]).toBe("max-age=63072000; includeSubDomains");
  } else if (!LIVE) {
    // next dev: HSTS would pin localhost to HTTPS.
    expect(first!.headers()["strict-transport-security"]).toBeUndefined();
  }
  expect(first!.headers()["x-frame-options"]).toBe("DENY");
  const second = await request.get("/login");
  expect(nonceOf(second.headers()["content-security-policy"]!)).not.toBe(nonceOf(csp));
  // Next stamped the nonce onto every inline script it server-rendered (checked on the raw HTML: browsers hide it).
  const html = await first!.text();
  const nonce = nonceOf(csp)!;
  const inline = [...html.matchAll(/<script(?![^>]*\ssrc=)[^>]*>/g)].map((m) => m[0]);
  expect(inline.length).toBeGreaterThan(0);
  for (const tag of inline) expect(tag).toContain(`nonce="${nonce}"`);
});

test("negative: un-nonced inline script code is blocked and the listener reports it", async ({ page, cspViolations }) => {
  await page.goto("/login");
  // An injected inline event handler is un-nonced inline script (a createElement script run from the test's own
  // context would be trusted through 'strict-dynamic', so it would not prove anything).
  await page.evaluate(() => {
    const div = document.createElement("div");
    div.innerHTML = '<img src="/csp-negative-test-missing.png" onerror="window.__injected = true">';
    document.body.appendChild(div);
  });
  await expect.poll(() => cspViolations.length).toBeGreaterThan(0);
  expect(await page.evaluate(() => (window as unknown as { __injected?: boolean }).__injected)).toBeUndefined();
  // Exactly the violation this test provoked (the inline handler is a `script-src-attr` violation); remove
  // only it so the fixture's final check still fails on any unrelated violation.
  expect(cspViolations).toEqual([expect.stringMatching(/^script-src-attr blocked inline /)]);
  cspViolations.splice(0, 1);
});

test.describe("mock pages", () => {
  test.skip(LIVE, "mock-mode flows");

  test("scoring-profile editor loads asd_wasm under the policy", async ({ page }) => {
    const wasmLoaded = page.waitForResponse((r) => r.url().includes("/asd-wasm/asd_wasm_bg.wasm"));
    await login(page, "superadmin@example.org");
    await page.getByLabel("Organisation id").fill(MOCK_ORG_ID);
    await page.getByRole("button", { name: "Switch" }).click();
    await page.getByRole("navigation", { name: "Main navigation" }).getByRole("link", { name: "Scoring", exact: true }).click();
    await expect(page.getByRole("heading", { name: "Scoring profile", exact: true })).toBeVisible();
    await page.getByRole("link", { name: /New draft from / }).click();
    await expect(page.getByText(/core unavailable/i)).toHaveCount(0);
    expect((await wasmLoaded).ok()).toBe(true);
  });
});
