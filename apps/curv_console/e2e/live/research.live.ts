/**
 * Live researcher toolkit e2e (phase 8: WP-X.2 experiments/cohorts, X.3 runs, X.5/X.7 console, X.8 export, X.9 sweeps
 * and age profiles) against `asd-backend serve` + `worker`. Run it with `e2e/live/run-research-live.sh`, which seeds
 * the org (seed-research.sh): a researcher, two superadmins, the published seeded config, research-consented
 * sessions at 30 months (some with a research frame log) and 3 at 54 months (a band below small_cell_k = 12).
 *
 * Serial: the steps share one experiment, cohort and run. Counts are asserted only where the seed fixes them.
 */
import { execFileSync } from "node:child_process";
import { mkdtempSync, readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import type { APIRequestContext, Cookie, Page } from "@playwright/test";
import { expect, test } from "../helpers";
import { apiToken, env, signIn } from "./support";

const RESEARCH_LIVE = Boolean(process.env.E2E_RESEARCH);
test.skip(!RESEARCH_LIVE, "set E2E_RESEARCH=1 (e2e/live/run-research-live.sh) to run against the researcher toolkit backend");
test.describe.configure({ mode: "serial" });

const org = () => env("E2E_ORG");
const researcher = () => env("E2E_RESEARCHER_EMAIL");
const api = () => env("E2E_API_URL");
const RUN_ID = Date.now();

/** Shared between the serial steps. */
const state: { experimentPath?: string; experimentId?: string; cohortName?: string; runPath?: string; promotedVersion?: string } = {};

/**
 * Sign-ins are rate limited per client (429 after about ten a minute), so each user signs in once and later steps
 * reuse the cookies; bearer tokens for the direct API checks are cached the same way.
 */
const sessions = new Map<string, Cookie[]>();
const tokens = new Map<string, string>();

async function reuse(page: Page, email: string, signInFresh: () => Promise<void>): Promise<void> {
  const saved = sessions.get(email);
  if (saved) {
    await page.context().addCookies(saved);
    await page.goto(`/${org()}/overview`);
    if (new URL(page.url()).pathname === `/${org()}/overview`) return;
  }
  await signInFresh();
  sessions.set(email, await page.context().cookies());
}

async function token(request: APIRequestContext, email: string): Promise<string> {
  if (!tokens.has(email)) tokens.set(email, await apiToken(request, email));
  return tokens.get(email)!;
}

async function asResearcher(page: Page): Promise<void> {
  await reuse(page, researcher(), () => signIn(page, researcher(), org()));
}

async function superadminIn(page: Page, email: string): Promise<void> {
  await reuse(page, email, () => superadminSignIn(page, email));
}

async function superadminSignIn(page: Page, email: string): Promise<void> {
  await page.goto("/login");
  await page.getByLabel("Email").fill(email);
  await page.getByLabel("Password").fill(env("E2E_PASSWORD"));
  await page.getByRole("button", { name: "Sign in" }).click();
  await expect(page.getByRole("heading", { name: "Choose an organisation" })).toBeVisible({ timeout: 120_000 });
  // The form may be clicked before React hydrates it: retry until the switch navigates.
  await expect(async () => {
    await page.getByLabel("Organisation id").fill(org());
    await page.getByRole("button", { name: "Switch" }).click();
    await expect(page).toHaveURL(new RegExp(`/${org()}/overview$`), { timeout: 5_000 });
  }).toPass({ timeout: 60_000 });
}

/** `analysis.read` audit rows for one run (owner reads the audit log). */
async function analysisReads(request: APIRequestContext, runId: string): Promise<number> {
  const ownerToken = await token(request, env("E2E_OWNER_EMAIL"));
  const res = await request.get(`${api()}/v1/orgs/${org()}/audit?action=analysis.read&target_id=${runId}&limit=100`, {
    headers: { Authorization: `Bearer ${ownerToken}` },
  });
  expect(res.status(), await res.text()).toBe(200);
  return ((await res.json()).items as unknown[]).length;
}

const runIdOf = (page: Page) => new URL(page.url()).pathname.split("/").pop()!;

/** Opens the run dialog on the experiment page, picks the saved cohort and the kind. */
async function openRunDialog(page: Page, kind: "cohort_rescore" | "param_sweep" | "age_profile") {
  await page.goto(state.experimentPath!);
  await page.getByRole("button", { name: "Run on cohort" }).click();
  const dialog = page.getByRole("dialog", { name: "Run on cohort" });
  await dialog.getByRole("combobox", { name: /^Cohort/ }).selectOption({ label: state.cohortName! });
  await dialog.getByRole("combobox", { name: /^Analysis/ }).selectOption(kind);
  return dialog;
}

async function startAndFinish(page: Page, dialog: ReturnType<Page["getByRole"]>): Promise<void> {
  await dialog.getByRole("button", { name: "Start run" }).click();
  await expect(page).toHaveURL(/\/research\/runs\/[^/]+$/, { timeout: 60_000 });
  await expect(page.getByTestId("run-status")).toHaveText("Finished", { timeout: 240_000 });
}

test("a researcher creates an experiment from the published profile; knobs move the preview at score and frames level", async ({ page, request }) => {
  await asResearcher(page);
  await page.goto(`/${org()}/research/experiments/new`);
  const name = `Live experiment ${RUN_ID}`;
  await page.getByLabel("Name", { exact: true }).fill(name);
  await page.getByRole("button", { name: "Create experiment" }).click();
  await expect(page.getByRole("heading", { name })).toBeVisible({ timeout: 60_000 });
  state.experimentPath = new URL(page.url()).pathname;
  state.experimentId = runIdOf(page);
  // The experiment starts from the published scoring profile (the runner published the seeded one; a re-run on the
  // same database finds the profile the previous run promoted and published).
  const published = await request.get(`${api()}/v1/config/scoring_profile?status=published&limit=1`, {
    headers: { Authorization: `Bearer ${await token(request, researcher())}` },
  });
  const publishedVersion = (await published.json()).items[0].version as string;
  await expect(page.getByText(`Based on profile ${publishedVersion}`, { exact: false })).toBeVisible();
  await expect(page.getByText("The Rust core accepts this profile.")).toBeVisible({ timeout: 60_000 });

  // Preview: the seeded frames session plus a score-only one.
  const picker = page.getByRole("group", { name: "Sessions to preview" });
  const checked = picker.getByRole("checkbox", { checked: true });
  await expect(checked.first()).toBeVisible();
  while ((await checked.count()) > 0) await checked.first().uncheck();
  const framesRow = picker.locator("div").filter({ has: page.getByText("frames", { exact: true }) }).filter({ has: page.getByRole("checkbox") });
  const scoreRow = picker.locator("div").filter({ has: page.getByText("score only", { exact: true }) }).filter({ has: page.getByRole("checkbox") });
  await framesRow.last().getByRole("checkbox").check();
  await scoreRow.last().getByRole("checkbox").check();
  const cards = page.getByRole("article");
  await expect(cards).toHaveCount(2, { timeout: 60_000 });
  await expect(page.getByText("The Rust core could not be loaded.")).toHaveCount(0);
  const scoreCard = cards.filter({ hasText: "score only" });
  const framesCard = cards.filter({ hasText: "frames" });
  for (const card of [scoreCard, framesCard]) {
    await expect(card.getByRole("region", { name: "Draft" }).getByLabel("Composite")).toHaveText(/\d/, { timeout: 60_000 });
  }

  // Score level: moving both floors to the other extreme relabels the draft side (a re-run on the same database starts
  // from the profile the previous run published, with the floors raised); the published side never moves.
  const publishedFlag = await scoreCard.getByRole("region", { name: "Published" }).getByText(/^Analysis label:/).innerText();
  const green = page.getByLabel("Green floor", { exact: true });
  const raise = Number(await green.inputValue()) < 0.9;
  await green.fill(raise ? "0.99" : "0.01");
  await page.getByLabel("Yellow floor", { exact: true }).fill(raise ? "0.98" : "0");
  await expect(scoreCard).toHaveAttribute("data-changed", "true", { timeout: 30_000 });
  await expect(scoreCard.getByRole("region", { name: "Published" }).getByText(/^Analysis label:/)).toHaveText(publishedFlag);

  // Frames level: the fixation threshold re-processes the research frame log (scanpath shown) of the frames card.
  await expect(framesCard.getByRole("region", { name: "Scanpath" })).toBeVisible();
  await expect(framesCard.getByRole("img", { name: /Gaze trace/ })).toBeVisible({ timeout: 60_000 });
  const traceBefore = await framesCard.getByRole("img", { name: /Gaze trace/ }).innerHTML();
  const ivt = page.getByLabel("Fixation velocity threshold", { exact: true });
  await ivt.fill(Number(await ivt.inputValue()) > 1 ? "0.6" : "4");
  await expect.poll(() => framesCard.getByRole("img", { name: /Gaze trace/ }).innerHTML(), { timeout: 60_000 }).not.toBe(traceBefore);
  await expect(framesCard).not.toContainText("param_not_applied: metrics.ivtVelocityUnitsPerSec");

  await page.getByRole("button", { name: "Save experiment" }).click();
  await expect(page.getByText("Saved.")).toBeVisible();
});

test("a cohort of research-consented sessions previews counts with small-cell suppression", async ({ page }) => {
  await asResearcher(page);
  await page.goto(`/${org()}/research/cohorts/new`);
  state.cohortName = `Live cohort ${RUN_ID}`;
  await page.getByLabel("Name", { exact: true }).fill(state.cohortName);
  await page.getByRole("button", { name: "Preview counts" }).click();
  const preview = page.getByTestId("cohort-preview");
  await expect(preview).toBeVisible();
  // 14 consented at 30 months (24-35), 3 at 54 months (48-60, below k = 12), 2 without research consent.
  await expect(preview).toContainText("24-35: 14");
  await expect(preview).toContainText("48-60: n < 12");
  await expect(preview).toContainText("2 without research consent");
  await page.getByRole("button", { name: "Save cohort" }).click();
  await expect(page).toHaveURL(/\/research\/cohorts\/[^/]+$/, { timeout: 60_000 });
  await expect(page.getByTestId("cohort-preview")).toContainText("n < 12");
});

test("cohort_rescore runs to succeeded; the page shows distributions, transitions and deltas; reads are audited once per view", async ({ page, request }) => {
  await asResearcher(page);
  const dialog = await openRunDialog(page, "cohort_rescore");
  await dialog.getByRole("button", { name: "Preview counts" }).click();
  await expect(dialog.getByRole("status")).toContainText("sessions");
  await startAndFinish(page, dialog);
  state.runPath = new URL(page.url()).pathname;
  const runId = runIdOf(page);

  await expect(page.getByRole("region", { name: "Provenance" })).toContainText("Definition hash");
  const results = page.getByRole("region", { name: "Results" });
  await expect(results).toBeVisible();
  // `auto` resolved the sessions with a research frame log at the frames level, the rest at the score level.
  await expect(results.getByRole("region", { name: "Sessions" })).toContainText(/Frame level: [1-9]\d*, Score level: [1-9]/);
  await expect(results.getByRole("table", { name: "Analysis label counts" })).toBeVisible();
  await expect(results.getByRole("table", { name: /Analysis label changes/ })).toBeVisible();
  await expect(results.getByRole("img", { name: /Change in the composite|composite/i }).first()).toBeVisible();
  await expect(results.getByRole("region", { name: "Age band 24-35" }).getByTestId("hist-bar").first()).toBeVisible();
  await expect(results.getByRole("region", { name: "Age band 48-60" })).toContainText("n < 12");

  const reads = await analysisReads(request, runId);
  expect(reads).toBeGreaterThanOrEqual(1);
  await page.reload();
  await expect(page.getByRole("region", { name: "Results" })).toBeVisible();
  await expect.poll(() => analysisReads(request, runId)).toBe(reads + 1);

  await page.getByRole("link", { name: "Per-session results" }).click();
  await expect(page.getByRole("table", { name: "Per-session results" })).toContainText("frames");
});

test("a one-axis parameter sweep started from the run dialog draws its curve", async ({ page }) => {
  await asResearcher(page);
  const dialog = await openRunDialog(page, "param_sweep");
  const axis = dialog.getByRole("group", { name: "Axis 1" });
  await axis.getByLabel("Parameter").selectOption("flag.greenFloor");
  await axis.getByLabel("From", { exact: true }).fill("0.5");
  await axis.getByLabel("To", { exact: true }).fill("0.8");
  await axis.getByLabel("Step", { exact: true }).fill("0.1");
  // 0.5, 0.6, 0.7, 0.8, plus the published floor when it falls inside the range off the grid (0.65 on a fresh seed).
  await expect(dialog.getByRole("status").filter({ hasText: "cells" })).toContainText(/^[45] cells/);
  await startAndFinish(page, dialog);
  const panel = page.getByRole("region", { name: "Parameter sweep" });
  await expect(panel).toBeVisible();
  await expect(panel.getByRole("img").first()).toBeVisible();
});

test("an age_profile run shows the shares per band and hides the small band", async ({ page }) => {
  await asResearcher(page);
  const dialog = await openRunDialog(page, "age_profile");
  await startAndFinish(page, dialog);
  const section = page.getByRole("region", { name: "Age profile" });
  await expect(section).toBeVisible();
  await expect(section.getByTestId("age-row").first()).toBeVisible();
  await expect(section.getByTestId("age-band-suppressed").first()).toHaveText("n < 12");
});

test("analysis_results export from the pre-filled form downloads a valid file", async ({ page }) => {
  await asResearcher(page);
  await page.goto(state.runPath!);
  await page.getByRole("link", { name: "Export results…" }).click();
  await expect(page).toHaveURL(/\/exports\?format=analysis_results&run=/);
  await expect(page.getByTestId("prefill-run")).toBeVisible();
  await page.getByLabel(/^Reason/).fill("Live e2e: compare the draft profile with the published one");
  await page.getByRole("button", { name: "Request export" }).click();
  await expect(page.getByText("Export queued.")).toBeVisible({ timeout: 60_000 });

  await page.goto(`/${org()}/exports`);
  const row = page.getByRole("row").filter({ hasText: "Analysis results" }).first();
  await expect(async () => {
    await page.reload();
    await expect(row).toContainText("Ready", { timeout: 2_000 });
  }).toPass({ timeout: 120_000 });
  const [download] = await Promise.all([page.waitForEvent("download"), row.getByRole("button", { name: "Download" }).click()]);
  const dir = mkdtempSync(join(tmpdir(), "research-live-"));
  const file = join(dir, download.suggestedFilename() || "export.bin");
  await download.saveAs(file);
  const bytes = readFileSync(file);
  if (bytes.subarray(0, 2).toString("latin1") === "PK") {
    const listing = execFileSync("unzip", ["-l", file], { encoding: "utf8" });
    expect(listing).toMatch(/manifest/i);
    execFileSync("unzip", ["-tq", file]);
    for (const entry of listing.split("\n").map((l) => l.trim().split(/\s+/).pop() ?? "").filter((n) => n.endsWith(".jsonl"))) {
      const text = execFileSync("unzip", ["-p", file, entry], { encoding: "utf8" });
      for (const line of text.split("\n").filter(Boolean)) JSON.parse(line);
    }
  } else {
    const lines = bytes.toString("utf8").split("\n").filter(Boolean);
    expect(lines.length).toBeGreaterThan(0);
    for (const line of lines) JSON.parse(line);
  }
});

test("promote to a draft profile; publishing an unanalysed human draft is refused, the promoted one is accepted", async ({ browser, request }) => {
  const rPage = await (await browser.newContext()).newPage();
  await asResearcher(rPage);
  await rPage.goto(state.experimentPath!);
  await rPage.getByRole("button", { name: "Promote to draft" }).click();
  state.promotedVersion = `live-promoted-${RUN_ID}`;
  await rPage.getByLabel("New scoring profile version").fill(state.promotedVersion);
  await rPage.getByLabel("Notes").fill("Live e2e promotion");
  await rPage.getByRole("button", { name: "Promote", exact: true }).click();
  await expect(rPage.getByText(`Promoted as scoring profile draft ${state.promotedVersion}.`)).toBeVisible({ timeout: 60_000 });

  // A human-authored draft with no analysis run over its body: the researcher files it through the API.
  const rToken = await token(request, researcher());
  const published = await request.get(`${api()}/v1/config/scoring_profile?status=published&limit=1`, { headers: { Authorization: `Bearer ${rToken}` } });
  // Any change makes a body hash no run has seen; nudge one knob within its range.
  const body = (await published.json()).items[0].body as { flag: { lowConfidenceShare: number } };
  const share = body.flag.lowConfidenceShare;
  body.flag.lowConfidenceShare = Math.round((share >= 0.9 ? share - 0.01 : share + 0.01) * 100) / 100;
  const unanalysed = `live-unanalysed-${RUN_ID}`;
  const created = await request.post(`${api()}/v1/config/scoring_profile`, {
    headers: { Authorization: `Bearer ${rToken}` },
    data: { version: unanalysed, body, notes: "Live e2e: never analysed" },
  });
  expect(created.status(), await created.text()).toBe(201);

  // Superadmin A approves both; superadmin B (four-eyes) publishes.
  const a = await (await browser.newContext()).newPage();
  await superadminIn(a, env("E2E_SUPERADMIN_EMAIL"));
  for (const version of [unanalysed, state.promotedVersion]) {
    await a.goto(`/${org()}/config/scoring/${version}`);
    await a.getByRole("button", { name: "Approve" }).click();
    await expect(a.getByRole("heading", { name: new RegExp(version) })).toContainText("Approved", { timeout: 30_000 });
  }

  const b = await (await browser.newContext()).newPage();
  await superadminIn(b, env("E2E_SUPERADMIN2_EMAIL"));
  for (const version of [unanalysed, state.promotedVersion]) {
    await b.goto(`/${org()}/config/scoring/${version}`);
    await b.getByRole("link", { name: "Run the sandbox with this version" }).click();
    await b.getByRole("button", { name: "Run re-score" }).click();
    await expect(b.getByRole("region", { name: "Summary" })).toBeVisible({ timeout: 60_000 });
    await b.goto(`/${org()}/config/scoring/${version}`);
    await b.getByRole("button", { name: "Publish" }).click();
    if (version === unanalysed) {
      await expect(b.getByRole("status").or(b.getByRole("alert")).filter({ hasText: "This scoring profile has no finished analysis run over it." })).toBeVisible();
    } else {
      await expect(b.getByText("Published. The previous published version was retired.")).toBeVisible();
    }
  }
});

test("superadmin cannot start analysis runs (GOV-1)", async ({ page, request }) => {
  const saToken = await token(request, env("E2E_SUPERADMIN_EMAIL"));
  const res = await request.post(`${api()}/v1/orgs/${org()}/analysis-runs`, {
    headers: { Authorization: `Bearer ${saToken}` },
    data: { kind: "cohort_rescore", experiment_id: state.experimentId, cohort: {}, level: "auto" },
  });
  expect(res.status(), await res.text()).toBe(403);
  await superadminIn(page, env("E2E_SUPERADMIN_EMAIL"));
  await page.goto(state.experimentPath!);
  await expect(page.getByRole("button", { name: "Run on cohort" })).toBeDisabled();
});

test("a long run watched for a minute writes no analysis.read rows; the cancel button cancels it", async ({ page, request }) => {
  test.setTimeout(300_000);
  await asResearcher(page);
  const dialog = await openRunDialog(page, "param_sweep");
  await dialog.getByRole("combobox", { name: /^Level/ }).selectOption("frames");
  await dialog.getByRole("button", { name: "Add a second axis" }).click();
  const axes: [string, string, string, string, string][] = [
    ["Axis 1", "qualityGate.minDistanceCm", "20", "44", "1"],
    ["Axis 2", "qualityGate.maxDistanceCm", "50", "74", "1"],
  ];
  for (const [group, id, from, to, step] of axes) {
    const axis = dialog.getByRole("group", { name: group });
    await axis.getByLabel("Parameter").selectOption(id);
    await axis.getByLabel("From", { exact: true }).fill(from);
    await axis.getByLabel("To", { exact: true }).fill(to);
    await axis.getByLabel("Step", { exact: true }).fill(step);
  }
  await dialog.getByRole("button", { name: "Start run" }).click();
  await expect(page).toHaveURL(/\/research\/runs\/[^/]+$/, { timeout: 60_000 });
  const runId = runIdOf(page);
  await expect(page.getByTestId("run-status")).toHaveText("Running", { timeout: 60_000 });
  await page.waitForTimeout(60_000);
  await expect(page.getByTestId("run-status")).toHaveText("Running");
  expect(await analysisReads(request, runId)).toBe(0);
  await page.getByRole("button", { name: "Cancel run" }).click();
  await expect(page.getByTestId("run-status")).toHaveText("Cancelled", { timeout: 60_000 });
  expect(await analysisReads(request, runId)).toBe(0);
});
