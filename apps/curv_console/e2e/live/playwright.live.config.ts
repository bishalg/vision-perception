/**
 * Live console smoke (scripts/live_smoke.sh). Runs only `*.live.ts` in this folder against a
 * console already running with CURV_API_MODE=live (E2E_BASE_URL). The default mock run
 * (`playwright.config.ts`, testDir ./e2e) matches only `*.spec.ts` / `*.test.ts`, so it never
 * picks these files up.
 */
import { defineConfig, devices } from "@playwright/test";

const baseURL = process.env.E2E_BASE_URL;
if (!baseURL) throw new Error("E2E_BASE_URL is required for the live console smoke");

export default defineConfig({
  testDir: ".",
  testMatch: "*.live.ts",
  fullyParallel: false,
  workers: 1,
  retries: 0,
  reporter: "list",
  // Dev-mode first compiles of each route can be slow.
  timeout: 240_000,
  expect: { timeout: 60_000 },
  outputDir: "../../test-results/live",
  use: { baseURL, trace: "retain-on-failure" },
  projects: [{ name: "chromium", use: { ...devices["Desktop Chrome"] } }],
});
