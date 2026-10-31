import { expect, type Page } from "@playwright/test";

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

/** Log in as the default user and wait for the org overview. Returns the org id from the URL. */
export async function loginToOverview(page: Page): Promise<string> {
  await login(page);
  await expect(page).toHaveURL(/\/[0-9a-f-]{36}\/overview$/);
  return new URL(page.url()).pathname.split("/")[1]!;
}
