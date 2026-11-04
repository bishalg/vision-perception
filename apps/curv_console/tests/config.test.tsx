/**
 * WP-C.5: baselines and scoring-profile editors, sandbox re-score, lifecycle gate and bundles. Components that use the
 * Rust core get the real asd-wasm build through `./asdCoreNode` (skipped when it is not built).
 */
import { paramText } from "@/lib/research/paramText";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { BundleCompose, parseFeatureFlags } from "@/components/config/BundleCompose";
import { DocLifecycle } from "@/components/config/DocLifecycle";
import { SandboxRunner } from "@/components/config/SandboxRunner";
import { ScoringProfileEditor } from "@/components/config/ScoringProfileEditor";
import { BaselinesEditor } from "@/components/config/BaselinesEditor";
import { ApiError, ROLES } from "@/lib/api";
import { createLiveClient } from "@/lib/api/live";
import { createMockClient, MOCK_ORG_ID } from "@/lib/api/mock";
import { MOCK_BASELINES_VERSIONS, MOCK_SCORING_VERSIONS, mockBaselineSet, mockScoringProfile } from "@/lib/api/mock/configSeed";
import {
  baselineProblemsByClip,
  diffDocuments,
  groupBaselineChanges,
  readSandboxRun,
  recordSandboxRun,
  summarizeSandbox,
} from "@/lib/configDocs";
import { navLinks } from "@/lib/nav";
import { can } from "@/lib/permissions";
import { configText } from "@/messages/config";
import { HAS_WASM } from "./asdCoreNode";

const push = vi.fn();
vi.mock("next/navigation", () => ({ useRouter: () => ({ push, back: vi.fn(), refresh: vi.fn() }) }));
vi.mock("@/lib/asd/browser", async () => {
  const { nodeCore: make } = await import("./asdCoreNode");
  const core = make();
  return { browserCore: () => core, ASD_WASM_BASE: "/asd-wasm/" };
});

const json = (body: unknown, status = 200) =>
  new Response(JSON.stringify(body), { status, headers: { "Content-Type": "application/json" } });

beforeEach(() => {
  push.mockReset();
  window.localStorage.clear();
});

describe("display helpers", () => {
  it("diffDocuments lists changed leaves; baseline changes group by clip", () => {
    const a = mockScoringProfile();
    const b = mockScoringProfile(true);
    const paths = diffDocuments(a, b).map((c) => c.path);
    expect(paths).toEqual(["flag.atypicalShareForRed", "flag.greenFloor", "flag.yellowFloor"]);
    expect(diffDocuments(a, structuredClone(a))).toEqual([]);
    const changes = diffDocuments(mockBaselineSet(), mockBaselineSet(true));
    const groups = groupBaselineChanges(changes);
    expect(groups.get("")!.map((c) => c.path)).toEqual(["provenance", "status"]);
    expect(groups.get("stim_01_social_geo_10s")!.some((c) => c.path.endsWith("byAge.default.expected"))).toBe(true);
  });

  it("groups the core's baseline problem lines by clip", () => {
    const m = baselineProblemsByClip(
      ["missing video profile: b", "a: no segments", "a/s/eye_dwell_ratio: no range for bracket 12-17", "odd line"],
      ["a"],
    );
    expect(m.get("a")).toHaveLength(2);
    expect(m.get("")).toEqual(["missing video profile: b", "odd line"]);
  });

  it("summarizes sandbox outcomes", () => {
    const s = summarizeSandbox([
      { sessionId: "1", before: { flag: "green", composite: 0.7 }, after: { flag: "yellow", composite: 0.5 } },
      { sessionId: "2", before: { flag: "red", composite: 0.2 }, after: { flag: "red", composite: 0.3 } },
      { sessionId: "3", before: null, after: { flag: "red", composite: 0.1 } },
    ]);
    expect(s).toMatchObject({ sessions: 3, changed: 1, failed: 1, transitions: { "green→yellow": 1 } });
    expect(s.meanDelta).toBeCloseTo(-0.05);
  });

  it("remembers sandbox runs per kind and version", () => {
    expect(readSandboxRun("scoring_profile", "v1")).toBeNull();
    recordSandboxRun("scoring_profile", "v1", { at: "2026-11-01T00:00:00Z", sessions: 3, changed: 1 });
    expect(readSandboxRun("scoring_profile", "v1")).toEqual({ at: "2026-11-01T00:00:00Z", sessions: 3, changed: 1 });
    expect(readSandboxRun("normative_baselines", "v1")).toBeNull();
  });

  it("parses feature flags", () => {
    expect(parseFeatureFlags("a=true\n\n b.c = false ")).toEqual({ flags: { a: true, "b.c": false } });
    expect(parseFeatureFlags("a=yes")).toEqual({ error: configText.bundles.flagsInvalid("a=yes") });
  });
});

describe("mock and live API", () => {
  it("seeds baselines and scoring profiles with a lifecycle, and composes bundles from the published ones", async () => {
    const m = createMockClient();
    const live = await m.listConfigDocuments("scoring_profile", { status: "published" });
    expect(live.items.map((d) => d.version)).toEqual([MOCK_SCORING_VERSIONS.published]);
    // Shape is checked; range rules are the Rust core's.
    await expect(m.createConfigDocument("scoring_profile", { version: "x1", body: { flag: {} } })).rejects.toBeInstanceOf(ApiError);
    const created = await m.createConfigDocument("normative_baselines", { version: "x1", body: mockBaselineSet(), notes: "n" });
    expect(created.status).toBe("draft");
    // The seeded draft was authored by someone else, so "you" may approve it.
    const approved = await m.transitionConfigDocument("normative_baselines", MOCK_BASELINES_VERSIONS.draft, "approve");
    expect(approved.status).toBe("candidate");
    const before = (await m.listConfigBundles()).items.length;
    const bundle = await m.composeConfigBundle({});
    expect(bundle.scoring_profile_version).toBe(MOCK_SCORING_VERSIONS.published);
    expect((await m.listConfigBundles()).items[0]!.bundle_version).toBe(bundle.bundle_version);
    expect((await m.listConfigBundles()).items.length).toBe(before + 1);
  });

  it("live client calls the bundle routes", async () => {
    const calls: { url: string; init?: RequestInit }[] = [];
    const summary = {
      id: "b1", bundle_version: "2026.11.01-1", catalog_version: "c", baselines_version: "b", scoring_profile_version: "s",
      key_id: "k", published_at: "2026-11-01T00:00:00Z",
    };
    const fetchImpl = vi.fn(async (url: string | URL | Request, init?: RequestInit) => {
      calls.push({ url: String(url), init });
      return String(url).includes("?") || init?.method !== "POST" ? json({ items: [summary], next_cursor: null }) : json(summary, 201);
    });
    const api = createLiveClient({ baseUrl: "http://x", accessToken: "t", fetchImpl: fetchImpl as never });
    expect((await api.listConfigBundles({ limit: 5 })).items).toHaveLength(1);
    expect(await api.composeConfigBundle({ org_id: "o", feature_flags: { a: true } })).toEqual(summary);
    expect(calls[0]!.url).toBe("http://x/v1/config/bundles?limit=5");
    expect(calls[1]!.url).toBe("http://x/v1/config/bundles");
    expect(JSON.parse(String(calls[1]!.init!.body))).toEqual({ org_id: "o", feature_flags: { a: true } });
  });
});

describe("RBAC and navigation", () => {
  it("bundles are composed by owner, admin and superadmin; config pages are in the nav for readers", () => {
    expect(ROLES.filter((r) => can(r, "config.publish"))).toEqual(["owner", "admin", "superadmin"]);
    const hrefs = navLinks("/o", "researcher").map((l) => l.href);
    for (const p of ["baselines", "scoring", "sandbox", "bundles"]) expect(hrefs).toContain(`/o/config/${p}`);
    expect(navLinks("/o", "viewer").some((l) => l.href === "/o/config/sandbox")).toBe(false);
  });
});

describe("lifecycle gate", () => {
  const hints = configText.scoring.lifecycle;

  it("publish waits for a sandbox run of this version in this browser", async () => {
    const run = vi.fn().mockResolvedValue({ ok: true, status: "published" });
    const { unmount } = render(
      <DocLifecycle kind="scoring_profile" version="v2" status="candidate" hints={hints} sandboxHref="/o/config/sandbox" run={run} />,
    );
    expect(screen.getByRole("button", { name: "Publish" })).toBeDisabled();
    expect(screen.getByText(configText.lifecycleGate.publishNeedsSandbox)).toBeInTheDocument();
    unmount();
    recordSandboxRun("scoring_profile", "v2", { at: "2026-11-01T10:00:00Z", sessions: 4, changed: 2 });
    render(<DocLifecycle kind="scoring_profile" version="v2" status="candidate" hints={hints} sandboxHref="/o/config/sandbox" run={run} />);
    const publish = screen.getByRole("button", { name: "Publish" });
    expect(publish).toBeEnabled();
    expect(screen.getByText(/4 sessions, 2 flags would change/)).toBeInTheDocument();
    await userEvent.click(publish);
    expect(run).toHaveBeenCalledWith("publish");
  });

  it("approve is not gated", () => {
    render(<DocLifecycle kind="normative_baselines" version="v3" status="draft" hints={configText.baselines.lifecycle} sandboxHref="/s" run={vi.fn()} />);
    expect(screen.getByRole("button", { name: "Approve" })).toBeEnabled();
  });
});

describe("bundle compose", () => {
  it("sends the scope and flags, and shows the server's reason", async () => {
    const compose = vi
      .fn()
      .mockResolvedValueOnce({ ok: false, slug: "bundle-signing-unavailable", status: 503 })
      .mockResolvedValueOnce({ ok: true, bundle: { bundle_version: "2026.11.01-2" } });
    render(<BundleCompose ready superadmin compose={compose} />);
    await userEvent.click(screen.getByLabelText(configText.bundles.scopeGlobal, { exact: false }));
    await userEvent.type(screen.getByLabelText(configText.bundles.flags), "beta=true");
    await userEvent.click(screen.getByRole("button", { name: configText.bundles.compose }));
    expect(await screen.findByText(configText.errors["bundle-signing-unavailable"]!)).toBeInTheDocument();
    expect(compose).toHaveBeenCalledWith({ global: true, featureFlags: { beta: true } });
    await userEvent.click(screen.getByRole("button", { name: configText.bundles.compose }));
    expect(await screen.findByText(configText.bundles.composed("2026.11.01-2"))).toBeInTheDocument();
  });

  it("is disabled until every kind is published", () => {
    render(<BundleCompose ready={false} superadmin={false} compose={vi.fn()} />);
    expect(screen.getByRole("button", { name: configText.bundles.compose })).toBeDisabled();
    expect(screen.queryByText(configText.bundles.scope)).toBeNull();
  });
});

describe.skipIf(!HAS_WASM)("editors and sandbox with the Rust core", () => {
  // The scoring editor renders the 88-control registry form; give jsdom and the first wasm load time.
  vi.setConfig({ testTimeout: 30_000 });
  const SLOW = { timeout: 15_000 };
  const noPreview = { sessions: [], published: null, load: vi.fn() };

  it("scoring profile editor shows the core's range problem at the field and blocks saving", async () => {
    const save = vi.fn().mockResolvedValue({ ok: true, version: "v9" });
    render(<ScoringProfileEditor orgId={MOCK_ORG_ID} initial={mockScoringProfile()} fromVersion="2026.10.1" preview={noPreview} save={save} />);
    expect(await screen.findByText(configText.editor.valid, {}, SLOW)).toBeInTheDocument();
    const green = screen.getByLabelText(paramText("en", "flag.greenFloor").label);
    await userEvent.clear(green);
    await userEvent.type(green, "1.5");
    await waitFor(() => expect(green).toHaveAttribute("aria-invalid", "true"), SLOW);
    expect((await screen.findAllByText("flag.greenFloor must be in [0, 1]", {}, SLOW)).length).toBeGreaterThan(0);

    await userEvent.type(screen.getByLabelText(/^New version/), "v9");
    await userEvent.type(screen.getByLabelText(/^Changelog note/), "stricter");
    await userEvent.click(screen.getByRole("button", { name: configText.editor.submit }));
    expect(save).not.toHaveBeenCalled();

    await userEvent.clear(green);
    await userEvent.type(green, "0.7");
    await screen.findByText(configText.editor.valid, {}, SLOW);
    await userEvent.click(screen.getByRole("button", { name: configText.editor.submit }));
    await waitFor(() => expect(save).toHaveBeenCalled(), SLOW);
    expect(save.mock.calls[0]![0]).toMatchObject({ version: "v9", notes: "stricter", body: { flag: { greenFloor: 0.7 } } });
    expect(push).toHaveBeenCalledWith(`/${MOCK_ORG_ID}/config/scoring/v9?created=1`);
  });

  it("scoring profile editor requires a changelog note", async () => {
    const save = vi.fn();
    render(<ScoringProfileEditor orgId="o" initial={mockScoringProfile()} fromVersion={null} preview={noPreview} save={save} />);
    await screen.findByText(configText.editor.valid, {}, SLOW);
    await userEvent.type(screen.getByLabelText(/^New version/), "v1");
    await userEvent.click(screen.getByRole("button", { name: configText.editor.submit }));
    expect(await screen.findByText(configText.editor.notesRequired)).toBeInTheDocument();
    expect(save).not.toHaveBeenCalled();
  });

  it("baselines editor marks the clip the core names and edits a cell", async () => {
    const set = mockBaselineSet();
    set.videos.stim_02_gaze_cue_8s!.segments = [];
    const save = vi.fn().mockResolvedValue({ ok: true, version: "b9" });
    render(<BaselinesEditor orgId="o" initial={set} fromVersion="2026.10.1" preview={noPreview} save={save} />);
    expect(await screen.findByText("stim_02_gaze_cue_8s: no segments", { selector: "li" })).toBeInTheDocument();
    const list = screen.getByRole("navigation", { name: configText.baselineEditor.clipList });
    expect(within(list).getByRole("button", { name: /stim_02_gaze_cue_8s/ })).toHaveTextContent(configText.baselineEditor.hasProblems);

    // Fix it by removing the clip, then edit the first clip's default eye-dwell value.
    await userEvent.click(within(list).getByRole("button", { name: /stim_02_gaze_cue_8s/ }));
    await userEvent.click(screen.getByRole("button", { name: configText.baselineEditor.removeClip }));
    await screen.findByText(configText.editor.valid);
    await userEvent.click(within(list).getByRole("button", { name: /stim_01_social_geo_10s/ }));
    const cell = screen.getByLabelText(`${configText.baselineEditor.expected} eye_dwell_ratio default`);
    await userEvent.clear(cell);
    await userEvent.type(cell, "0.4");
    await userEvent.type(screen.getByLabelText(/^New version/), "b9");
    await userEvent.type(screen.getByLabelText(/^Changelog note/), "eye norms");
    await userEvent.click(screen.getByRole("button", { name: configText.editor.submit }));
    await waitFor(() => expect(save).toHaveBeenCalled());
    const body = save.mock.calls[0]![0].body;
    expect(body.videos.stim_02_gaze_cue_8s).toBeUndefined();
    expect(body.videos.stim_01_social_geo_10s.segments[0].expectations[1].byAge.default.expected).toBe(0.4);
  });

  it("sandbox re-scores sessions and records the run for the draft versions", async () => {
    const m = createMockClient();
    const sessions = (await m.listSessions(MOCK_ORG_ID, { limit: 8 })).items;
    const load = vi.fn(async (ids: string[]) => ({
      ok: true as const,
      sessions: await Promise.all(ids.map(async (id) => ({ id, record: (await m.getSessionRaw(id)).raw_payload }))),
    }));
    render(
      <SandboxRunner
        sessions={sessions}
        initialSelected={sessions.map((x) => x.id)}
        published={{ label: "pub", profile: mockScoringProfile(), baselines: mockBaselineSet() }}
        draft={{ label: "draft", profile: mockScoringProfile(true), baselines: mockBaselineSet(true) }}
        load={load}
        record={[{ kind: "scoring_profile", version: "rc1" }]}
      />,
    );
    expect(screen.getByText(/lowConfidenceValidRatio is not applied/)).toBeInTheDocument();
    expect(screen.getByText(/"unreliable" flag is not reproduced/)).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: configText.sandbox.run }));
    const summary = await screen.findByRole("region", { name: configText.sandbox.summary.title });
    expect(load).toHaveBeenCalledWith(sessions.map((x) => x.id));
    const changed = Number(within(summary).getByTestId("flags-changed").textContent);
    expect(changed).toBeGreaterThan(0);
    const rows = within(screen.getByRole("table", { name: configText.sandbox.title })).getAllByRole("row");
    expect(rows).toHaveLength(sessions.length + 1);
    expect(readSandboxRun("scoring_profile", "rc1")).toMatchObject({ sessions: sessions.length, changed });
  });

  it("sandbox asks for at least one session", async () => {
    render(
      <SandboxRunner sessions={[]} initialSelected={[]} published={{ label: "p", profile: null, baselines: mockBaselineSet() }}
        draft={{ label: "d", profile: null, baselines: mockBaselineSet() }} load={vi.fn()} />,
    );
    await userEvent.click(screen.getByRole("button", { name: configText.sandbox.run }));
    expect(await screen.findByText(configText.sandbox.pickSessions)).toBeInTheDocument();
  });
});
