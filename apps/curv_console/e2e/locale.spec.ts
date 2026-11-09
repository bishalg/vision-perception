/**
 * WP-L.1.0 (I18N-16, I18N-21; acceptance 6, 9): `<html lang>` follows the mock user's locale, and the self-hosted
 * Devanagari font loads without a CSP violation (the `cspViolations` fixture in `helpers.ts` fails the test on one).
 * Mock mode: a `+ne` tag on the email's local part seeds the user's own locale (`clinician+ne@example.org`).
 */
import { expect, test, LIVE, login, loginToOverview, OVERVIEW_URL } from "./helpers";

test.skip(LIVE, "locale flows use the mock backend's user locale seed");

test("a user whose locale is ne gets lang=ne-NP and an en user gets lang=en", async ({ page }) => {
  await login(page, "clinician+ne@example.org");
  await expect(page).toHaveURL(OVERVIEW_URL);
  await expect(page.locator("html")).toHaveAttribute("lang", "ne-NP");

  await page.context().clearCookies();
  await loginToOverview(page);
  await expect(page.locator("html")).toHaveAttribute("lang", "en");
});

test("the login page follows Accept-Language when there is no session", async ({ browser }) => {
  const ne = await browser.newContext({ locale: "ne-NP", extraHTTPHeaders: { "Accept-Language": "ne-NP,en;q=0.5" } });
  const page = await ne.newPage();
  await page.goto("/login");
  await expect(page.locator("html")).toHaveAttribute("lang", "ne-NP");
  await ne.close();

  const fr = await browser.newContext({ extraHTTPHeaders: { "Accept-Language": "fr" } });
  const frPage = await fr.newPage();
  await frPage.goto("/login");
  await expect(frPage.locator("html")).toHaveAttribute("lang", "en");
  await fr.close();
});

test("the self-hosted Devanagari font is served from this origin under the strict CSP", async ({ page }) => {
  const fonts: string[] = [];
  page.on("response", (r) => {
    if (/\.woff2(\?|$)/.test(r.url())) fonts.push(r.url());
  });
  await login(page, "clinician+ne@example.org");
  await expect(page).toHaveURL(OVERVIEW_URL);
  await expect(page.locator("html")).toHaveAttribute("lang", "ne-NP");
  for (const url of fonts) expect(new URL(url).origin).toBe(new URL(page.url()).origin);
  // The face is declared for the page (loaded lazily, only when Devanagari text is rendered).
  const declared = await page.evaluate(() => [...document.fonts].map((f) => f.family));
  expect(declared.join(" ")).toMatch(/Devanagari|__.*font/i);
});

// Acceptance 9: enabled at integration, when every page of the wave renders in Nepali.
test.fixme("crawl: no visible text node of three or more ASCII letters on any nav page in ne", async () => {
  // Visit every nav page and every tab of a session, a participant and a research run as `clinician+ne@…`, collect
  // text nodes matching /[A-Za-z]{3,}/ and compare with the allowlist (brand names, units, MRN/TOTP/CSV/FHIR, seed data).
});
