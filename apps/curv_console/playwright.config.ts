/**
 * Playwright smoke suite (spec 03 §5: login, session detail, review action; plus devices enrol code).
 *
 * Default: starts `next dev` on port 3100 in mock mode (no backend needed).
 * Live:    E2E_BASE_URL=http://localhost:3000 E2E_EMAIL=... E2E_PASSWORD=... npm run e2e
 *          against a console already running with CURV_API_MODE=live (see README "Live mode").
 */
import { defineConfig, devices } from "@playwright/test";

const PORT = Number(process.env.E2E_PORT ?? 3100);
const external = process.env.E2E_BASE_URL;

export default defineConfig({
  testDir: "./e2e",
  fullyParallel: false,
  // The mock backend is one in-memory process and triage mutates it: run serially.
  workers: 1,
  retries: process.env.CI ? 1 : 0,
  forbidOnly: Boolean(process.env.CI),
  reporter: process.env.CI ? [["list"], ["html", { open: "never" }]] : "list",
  timeout: 60_000,
  expect: { timeout: 15_000 },
  use: {
    baseURL: external ?? `http://localhost:${PORT}`,
    trace: "retain-on-failure",
  },
  projects: [{ name: "chromium", use: { ...devices["Desktop Chrome"] } }],
  webServer: external
    ? undefined
    : {
        command: `npx next dev --port ${PORT}`,
        url: `http://localhost:${PORT}/login`,
        reuseExistingServer: !process.env.CI,
        timeout: 180_000,
        env: { CURV_API_MODE: "mock", CURV_COOKIE_SECURE: "false", NEXT_TELEMETRY_DISABLED: "1" },
      },
});
