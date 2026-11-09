import { expect, test as base, type Page } from "@playwright/test";
import { pathsFor, type OrgPaths } from "../src/lib/paths";

/**
 * CSP-3: every page a test opens reports `securitypolicyviolation` events (page and worker-free main frame; the
 * listener is installed before any page script runs, so it also sees violations during load and after navigations).
 * A violation fails the test, so a policy that blocks `asd_wasm` cannot hide behind the editors' silent "core
 * unavailable" fallback. A test that provokes a violation on purpose drains `cspViolations` itself.
 *
 * All specs import `test` from here instead of `@playwright/test`.
 */
export const test = base.extend<{ cspViolations: string[] }>({
  cspViolations: async ({}, provide) => {
    const seen: string[] = [];
    await provide(seen);
    expect(seen, "Content-Security-Policy violations").toEqual([]);
  },
  page: async ({ page, cspViolations }, provide) => {
    await page.exposeBinding("__reportCspViolation", (_source, detail: string) => {
      cspViolations.push(detail);
    });
    await page.addInitScript(() => {
      document.addEventListener("securitypolicyviolation", (e) => {
        const report = (window as unknown as { __reportCspViolation?: (d: string) => void }).__reportCspViolation;
        report?.(`${e.violatedDirective} blocked ${e.blockedURI || "inline"} (${e.sourceFile || "page"}:${e.lineNumber})`);
      });
    });
    await provide(page);
    // Violation events reach `cspViolations` asynchronously through the exposed binding: let a late one land
    // before the `cspViolations` fixture asserts (this teardown runs first, while the page is still open).
    if (!page.isClosed()) {
      await page.evaluate(() => new Promise((r) => setTimeout(r, 50))).catch(() => undefined);
    }
  },
});
export { expect };


/** Mock mode accepts any email with password "demo"; live runs pass real credentials via env. */
export const LIVE = Boolean(process.env.E2E_BASE_URL);
export const EMAIL = process.env.E2E_EMAIL ?? "demo@example.org";
export const PASSWORD = process.env.E2E_PASSWORD ?? "demo";
export const MOCK_ORG_ID = "5f0e1d2c-3b4a-4968-8776-655443322110";
/** Superadmin login and an org to open (live runs set these; mock uses its built-in superadmin). */
export const SUPERADMIN_EMAIL = process.env.E2E_SUPERADMIN_EMAIL ?? (LIVE ? undefined : "superadmin@example.org");
export const SUPERADMIN_PASSWORD = process.env.E2E_SUPERADMIN_PASSWORD ?? PASSWORD;
export const TARGET_ORG_ID = process.env.E2E_ORG_ID ?? MOCK_ORG_ID;

export async function login(page: Page, email = EMAIL, password = PASSWORD): Promise<void> {
  await page.goto("/login");
  await page.getByLabel("Email").fill(email);
  await page.getByLabel("Password").fill(password);
  await page.getByRole("button", { name: "Sign in" }).click();
}

/** WP-C.23 (N-4): every console URL in a spec comes from `src/lib/paths.ts` (`paths(org).clinic.sessions`, ...). */
export function paths(org: string): OrgPaths {
  return pathsFor(org);
}

/** The landing page after sign-in: `/{org}/clinic/overview`. */
export const OVERVIEW_URL = /\/[0-9a-f-]{36}\/clinic\/overview$/;

/** Top-bar division labels (spec 03 §3a). */
export type DivisionName = "Clinic" | "Research" | "Configuration" | "Data" | "Administration";

/** The top bar's division links. */
export const mainNav = (page: Page) => page.getByRole("navigation", { name: "Main navigation" });

/** The sidebar of a division (its `nav` is labelled with the division's name). */
export const sideNav = (page: Page, division: DivisionName) => page.getByRole("navigation", { name: division, exact: true });

/** Open a page the way a user does: the division in the top bar, then the item in its sidebar. */
export async function navTo(page: Page, division: DivisionName, item: string): Promise<void> {
  const divisionLink = mainNav(page).getByRole("link", { name: division, exact: true });
  await divisionLink.click();
  await expect(divisionLink).toHaveAttribute("aria-current", "true");
  const link = sideNav(page, division).getByRole("link", { name: item, exact: true });
  const href = (await link.getAttribute("href"))!;
  await link.click();
  // Wait for the client navigation, so the next step acts on the new page.
  await expect(page).toHaveURL((u) => u.pathname === href);
  await expect(link).toHaveAttribute("aria-current", "page");
}

/** Open the Account menu (top right): Account security and Sign out live there. */
export async function openAccountMenu(page: Page): Promise<void> {
  await page.locator("summary", { hasText: /^Account$/ }).click();
}

/** Log in as the default user and wait for the org overview. Returns the org id from the URL. */
export async function loginToOverview(page: Page): Promise<string> {
  await login(page);
  await expect(page).toHaveURL(OVERVIEW_URL);
  return new URL(page.url()).pathname.split("/")[1]!;
}
