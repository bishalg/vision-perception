/** Shared steps of the live record and share specs (`run-record-live.sh` sets the environment). */
import { readFileSync } from "node:fs";
import type { APIRequestContext, Page } from "@playwright/test";
import { expect } from "../helpers";

export const env = (name: string): string => {
  const v = process.env[name];
  if (!v) throw new Error(`${name} is required (set by e2e/live/run-record-live.sh)`);
  return v;
};

export const RECORD_LIVE = Boolean(process.env.E2E_RECORD);
export const SKIP_REASON = "set E2E_RECORD=1 (e2e/live/run-record-live.sh) to run against a backend with WP-B.19 / WP-I.13";

/** A local calendar day `n` days from now, as a date input takes it. */
export function day(n: number): string {
  const d = new Date(Date.now() + n * 86_400_000);
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
}

export async function signIn(page: Page, email: string, org: string): Promise<void> {
  await page.goto("/login");
  await page.getByLabel("Email").fill(email);
  await page.getByLabel("Password").fill(env("E2E_PASSWORD"));
  await page.getByRole("button", { name: "Sign in" }).click();
  await expect(page).toHaveURL(new RegExp(`/${org}/overview$`), { timeout: 120_000 });
}

/** A bearer token straight from the backend, for checks the console does not show (e.g. an export's sha256). */
export async function apiToken(request: APIRequestContext, email: string): Promise<string> {
  const res = await request.post(`${env("E2E_API_URL")}/v1/auth/login`, { data: { email, password: env("E2E_PASSWORD") } });
  expect(res.status()).toBe(200);
  return (await res.json()).access_token as string;
}

/** The device posts the 1.7 fixture as a new session of `token` (what the iPad app does after a capture). */
export async function ingestSession(request: APIRequestContext, token: string): Promise<string> {
  const fixture = JSON.parse(readFileSync(env("E2E_SESSION_FIXTURE"), "utf8"));
  const sessionId = `LIVE-REC-${Date.now()}-${Math.floor(Math.random() * 1e6)}`;
  fixture.sessionId = sessionId;
  fixture.participantToken = token;
  fixture.rawPayload.sessionId = sessionId;
  fixture.rawPayload.participantId = token;
  const res = await request.post(`${env("E2E_API_URL")}/v1/sessions`, {
    headers: {
      Authorization: `Bearer ${env("E2E_DEVICE_TOKEN")}`,
      "X-Schema-Version": "1.7",
      "X-Idempotency-Key": sessionId,
    },
    data: fixture,
  });
  expect(res.status(), await res.text()).toBe(201);
  return sessionId;
}

/** Create a participant from the list page (code + QR shown once) and open its record. */
export async function createParticipant(page: Page, org: string): Promise<{ token: string; path: string }> {
  await page.goto(`/${org}/participants`);
  await page.getByRole("button", { name: "New participant" }).click();
  await page.getByRole("button", { name: "Create participant" }).click();
  const token = (await page.getByTestId("created-token").innerText()).trim();
  expect(token).toMatch(/^child-[0-9a-f]{20}$/);
  await expect(page.getByRole("img", { name: `QR code for participant ${token}` })).toBeVisible();
  await page.getByRole("link", { name: "Open record" }).click();
  await expect(page).toHaveURL(new RegExp(`/${org}/participants/[0-9a-f-]{36}$`));
  return { token, path: new URL(page.url()).pathname };
}
