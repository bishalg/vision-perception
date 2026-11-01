/**
 * Playwright smoke suite (spec 03 §5: login, session detail, review action; plus devices enrol code).
 *
 * Default (`npm run e2e`): two projects, both in mock mode (no backend needed):
 * - `chromium` runs every spec against `next dev` on port 3100;
 * - `prod` runs the CSP specs (`csp.spec.ts`, `csp-prod.spec.ts`) against a production build (`next build` +
 *   `next start`) on port 3200. Only that build has the real policy (no `'unsafe-eval'`, HSTS) and serves any
 *   prerendered HTML, so this is the project that proves the nonce CSP holds in production (WP-C.16 CSP-1..4).
 *   `E2E_PROD_REUSE_BUILD=1` starts the existing `.next` build instead of rebuilding (local iteration only).
 * Live:    E2E_BASE_URL=http://localhost:3000 E2E_EMAIL=... E2E_PASSWORD=... npm run e2e
 *          against a console already running with CURV_API_MODE=live (see README "Live mode"); `chromium` only.
 */
import { defineConfig, devices } from "@playwright/test";

const PORT = Number(process.env.E2E_PORT ?? 3100);
const PROD_PORT = Number(process.env.E2E_PROD_PORT ?? 3200);
const external = process.env.E2E_BASE_URL;
const PROD_SPECS = ["csp.spec.ts", "csp-prod.spec.ts"];
const mockEnv = { CURV_API_MODE: "mock", CURV_COOKIE_SECURE: "false", NEXT_TELEMETRY_DISABLED: "1" };
const prodBuild = process.env.E2E_PROD_REUSE_BUILD ? "true" : "npm run build";

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
  use: { trace: "retain-on-failure" },
  projects: [
    {
      name: "chromium",
      testIgnore: ["csp-prod.spec.ts"],
      use: { ...devices["Desktop Chrome"], baseURL: external ?? `http://localhost:${PORT}` },
    },
    ...(external
      ? []
      : [
          {
            name: "prod",
            testMatch: PROD_SPECS,
            use: { ...devices["Desktop Chrome"], baseURL: `http://localhost:${PROD_PORT}` },
          },
        ]),
  ],
  webServer: external
    ? undefined
    : [
        {
          command: `node scripts/prepare-asd-wasm.mjs && npx next dev --port ${PORT}`,
          url: `http://localhost:${PORT}/login`,
          reuseExistingServer: !process.env.CI,
          timeout: 180_000,
          env: mockEnv,
        },
        {
          // `npm run build` runs the asd-wasm copy (prebuild) and the static-HTML CSP check (postbuild).
          command: `${prodBuild} && npx next start --port ${PROD_PORT}`,
          url: `http://localhost:${PROD_PORT}/login`,
          reuseExistingServer: !process.env.CI,
          timeout: 900_000,
          env: mockEnv,
        },
      ],
});
