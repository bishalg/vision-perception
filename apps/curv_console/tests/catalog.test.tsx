/** WP-C.4: catalog editor. Wire format, mock lifecycle rules, editor helpers, components and RBAC. */
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { BatteryPanel } from "@/components/catalog/BatteryPanel";
import { CatalogEditor } from "@/components/catalog/CatalogEditor";
import { CatalogDiffView } from "@/components/catalog/CatalogTables";
import { LifecycleActions, stepsFor } from "@/components/catalog/LifecycleActions";
import { ApiError, ROLES } from "@/lib/api";
import type { StimulusCatalog } from "@/lib/api/catalog";
import { createLiveClient } from "@/lib/api/live";
import { createMockClient } from "@/lib/api/mock";
import { MOCK_CATALOG_VERSIONS } from "@/lib/api/mock/catalog";
import { SEED_CATALOG } from "@/lib/api/mock/catalogSeed";
import {
  areasAt,
  batteriesOf,
  diffCatalogs,
  fieldPath,
  moveBox,
  resizeBox,
  serverFieldErrors,
  setBattery,
  shapeErrors,
} from "@/lib/catalog";
import { navItems } from "@/lib/nav";
import { can } from "@/lib/permissions";
import { catalogText } from "@/messages/catalog";

const push = vi.fn();
vi.mock("next/navigation", () => ({ useRouter: () => ({ push, refresh: vi.fn() }) }));

const json = (body: unknown, status = 200) =>
  new Response(JSON.stringify(body), { status, headers: { "Content-Type": "application/json" } });

const clone = <T,>(v: T): T => JSON.parse(JSON.stringify(v)) as T;

const small = (): StimulusCatalog => ({
  schemaVersion: "1.0.0",
  languages: ["en"],
  fallbackLanguage: "en",
  agePacks: [{ id: "age_12_24", minMonths: 0, maxMonths: 23 }],
  clips: [
    {
      id: "a", title: "Clip A", category: "Social", durationSec: 8, minMonths: 12, maxMonths: 60,
      protocols: [{ id: "express_30s", order: 1 }],
      areas: [{ name: "Face", startSec: 0, endSec: 4, xMin: 0.2, yMin: 0.2, xMax: 0.6, yMax: 0.6, roles: ["social"] }],
    },
    {
      id: "b", title: "Clip B", category: "Geo", durationSec: 6, minMonths: 12, maxMonths: 60,
      protocols: [{ id: "express_30s", order: 0 }, { id: "comprehensive_60s", order: 0 }],
      areas: [],
    },
  ],
});

const doc = (over: Record<string, unknown> = {}) => ({
  id: "d1", kind: "stimulus_catalog", version: "2026.11.0", status: "draft", body: small(), created_by: "u1",
  created_at: "2026-10-31T09:00:00.000Z", published_at: null, notes: null, ...over,
});

describe("live client wire format", () => {
  function client(response: Response) {
    const f = vi.fn().mockResolvedValue(response);
    return { f, api: createLiveClient({ baseUrl: "https://api.test", accessToken: "tok", fetchImpl: f as unknown as typeof fetch }) };
  }

  it("lists a kind with status, cursor and limit", async () => {
    const { f, api } = client(json({ items: [doc()], next_cursor: null, total: null }));
    const page = await api.listConfigDocuments("stimulus_catalog", { status: "draft", cursor: "c", limit: 50 });
    expect(f.mock.calls[0]![0]).toBe("https://api.test/v1/config/stimulus_catalog?status=draft&cursor=c&limit=50");
    expect(page.items[0]!.version).toBe("2026.11.0");
  });

  it("gets a version, creates a draft with POST and runs a transition", async () => {
    const get = client(json(doc()));
    await get.api.getConfigDocument("stimulus_catalog", "2026.11.0");
    expect(get.f.mock.calls[0]![0]).toBe("https://api.test/v1/config/stimulus_catalog/2026.11.0");

    const create = client(json(doc(), 201));
    await create.api.createConfigDocument("stimulus_catalog", { version: "2026.11.0", body: small(), notes: "n" });
    const [url, init] = create.f.mock.calls[0]!;
    expect(url).toBe("https://api.test/v1/config/stimulus_catalog");
    expect(init.method).toBe("POST");
    expect(JSON.parse(init.body)).toEqual({ version: "2026.11.0", body: small(), notes: "n" });

    const step = client(json(doc({ status: "candidate" })));
    const r = await step.api.transitionConfigDocument("stimulus_catalog", "2026.11.0", "approve");
    expect(step.f.mock.calls[0]![0]).toBe("https://api.test/v1/config/stimulus_catalog/2026.11.0/approve");
    expect(step.f.mock.calls[0]![1].method).toBe("POST");
    expect(r.status).toBe("candidate");
  });

  it("surfaces the problem+json of a rejected draft", async () => {
    const { api } = client(
      json({ type: "https://schemas.curv.org/asd/problems/validation-failed", title: "Validation failed", status: 400,
        invalid_params: [{ name: "body", reason: "clips must not be empty" }] }, 400),
    );
    const err = await api.createConfigDocument("stimulus_catalog", { version: "x", body: {} }).catch((e) => e);
    expect(err).toBeInstanceOf(ApiError);
    expect(err.slug).toBe("validation-failed");
    expect(err.problem.invalid_params[0].reason).toBe("clips must not be empty");
  });
});

describe("mock lifecycle", () => {
  let api: ReturnType<typeof createMockClient>;
  beforeEach(() => {
    api = createMockClient();
  });
  const err = (p: Promise<unknown>) => p.then(() => null, (e: ApiError) => e);

  it("seeds one document per status, newest first", async () => {
    const page = await api.listConfigDocuments("stimulus_catalog");
    expect(page.items.map((d) => d.version)).toEqual([
      MOCK_CATALOG_VERSIONS.draft, MOCK_CATALOG_VERSIONS.candidate, MOCK_CATALOG_VERSIONS.published, MOCK_CATALOG_VERSIONS.bundled,
    ]);
    const published = await api.listConfigDocuments("stimulus_catalog", { status: "published" });
    expect(published.items.map((d) => d.version)).toEqual([MOCK_CATALOG_VERSIONS.published]);
    // WP-C.5 seeds the other kinds separately; the catalog list is not mixed with them.
    expect((await api.listConfigDocuments("scoring_profile")).items.every((d) => d.kind === "scoring_profile")).toBe(true);
  });

  it("approves and publishes someone else's draft; publishing retires the previous one", async () => {
    expect((await api.transitionConfigDocument("stimulus_catalog", MOCK_CATALOG_VERSIONS.draft, "approve")).status).toBe("candidate");
    expect((await api.transitionConfigDocument("stimulus_catalog", MOCK_CATALOG_VERSIONS.draft, "publish")).status).toBe("published");
    expect((await api.getConfigDocument("stimulus_catalog", MOCK_CATALOG_VERSIONS.published)).status).toBe("retired");
  });

  it("enforces state and the four-eyes rule on the user's own draft", async () => {
    await api.createConfigDocument("stimulus_catalog", { version: "mine-1", body: small() });
    const e1 = await err(api.transitionConfigDocument("stimulus_catalog", "mine-1", "approve"));
    expect([e1?.status, e1?.slug]).toEqual([403, "four-eyes-required"]);
    const e2 = await err(api.transitionConfigDocument("stimulus_catalog", MOCK_CATALOG_VERSIONS.published, "approve"));
    expect([e2?.status, e2?.slug]).toEqual([409, "invalid-transition"]);
    expect((await api.transitionConfigDocument("stimulus_catalog", "mine-1", "retire")).status).toBe("retired");
  });

  it("rejects a bad version, an empty catalog and a taken version", async () => {
    const bad = await err(api.createConfigDocument("stimulus_catalog", { version: "a b", body: small() }));
    expect(bad?.problem?.invalid_params?.[0]?.name).toBe("version");
    const empty = await err(api.createConfigDocument("stimulus_catalog", { version: "e1", body: { ...small(), clips: [] } }));
    expect(empty?.problem?.invalid_params).toEqual([{ name: "body", reason: "clips must not be empty" }]);
    const taken = await err(api.createConfigDocument("stimulus_catalog", { version: MOCK_CATALOG_VERSIONS.draft, body: small() }));
    expect([taken?.status, taken?.slug]).toEqual([409, "config-version-exists"]);
  });
});

describe("editor helpers", () => {
  const m = catalogText.shape;

  it("shape errors: required text, numbers and integers, keyed by field path", () => {
    const c = small();
    c.clips[0]!.title = "";
    c.clips[0]!.durationSec = NaN;
    c.clips[1]!.minMonths = 1.5;
    c.clips[0]!.areas[0]!.name = " ";
    const e = shapeErrors(c, "", m);
    expect(e).toMatchObject({
      version: m.required,
      "clips.0.title": m.required,
      "clips.0.durationSec": m.number,
      "clips.1.minMonths": m.integer,
      "clips.0.areas.0.name": m.required,
    });
    expect(shapeErrors(small(), "1.0", m)).toEqual({});
  });

  it("does not add rules beyond shape (ranges and ordering are the server's)", () => {
    const c = small();
    Object.assign(c.clips[0]!.areas[0]!, { xMin: 1.4, startSec: 9, endSec: 2 });
    c.clips[0]!.maxMonths = 1;
    expect(shapeErrors(c, "1.0", m)).toEqual({});
  });

  it("maps problem+json names to field paths", () => {
    expect(fieldPath("body.clips[2].title")).toBe("clips.2.title");
    expect(fieldPath("/clips/0/areas/1/xMin")).toBe("clips.0.areas.1.xMin");
    expect(fieldPath("version")).toBe("version");
    expect(serverFieldErrors([
      { name: "body", reason: "clips must not be empty" },
      { name: "version", reason: "bad" },
      { name: "body.clips[1].id", reason: "duplicate" },
    ])).toEqual({ fields: { version: "bad", "clips.1.id": "duplicate" }, general: ["clips must not be empty"] });
  });

  it("batteries group memberships by order and write positions back", () => {
    const c = small();
    expect(batteriesOf(c)).toEqual([
      { id: "comprehensive_60s", clipIds: ["b"] },
      { id: "express_30s", clipIds: ["b", "a"] },
    ]);
    const next = setBattery(c, "express_30s", ["a"]);
    expect(next.clips[0]!.protocols).toEqual([{ id: "express_30s", order: 0 }]);
    expect(next.clips[1]!.protocols).toEqual([{ id: "comprehensive_60s", order: 0 }]);
    const added = setBattery(next, "comprehensive_60s", ["b", "a"]);
    expect(added.clips[0]!.protocols).toEqual([{ id: "express_30s", order: 0 }, { id: "comprehensive_60s", order: 1 }]);
    expect(batteriesOf(SEED_CATALOG).map((b) => b.id)).toEqual(["comprehensive_60s", "express_30s"]);
  });

  it("box geometry stays inside the frame", () => {
    const a = small().clips[0]!.areas[0]!;
    expect(moveBox(a, 0.9, -0.9)).toMatchObject({ xMin: 0.6, xMax: 1, yMin: 0, yMax: 0.4 });
    expect(resizeBox(a, 2, -2)).toMatchObject({ xMax: 1, yMax: 0.22 });
    expect(areasAt(small().clips[0]!, 2)).toEqual([0]);
    expect(areasAt(small().clips[0]!, 5)).toEqual([]);
  });

  it("diff by clip id", () => {
    const a = small();
    const b = clone(a);
    b.clips[0]!.title = "Renamed";
    b.clips.splice(1, 1);
    b.clips.push({ ...clone(a.clips[1]!), id: "c" });
    b.languages = ["en", "hi"];
    expect(diffCatalogs(a, b)).toEqual({
      fields: ["languages"], added: ["c"], removed: ["b"], changed: [{ id: "a", fields: ["title"] }],
    });
  });
});

describe("components", () => {
  beforeEach(() => push.mockReset());

  it("editor blocks a draft with shape errors and shows them at the fields", async () => {
    const save = vi.fn();
    render(<CatalogEditor orgId="o" initial={small()} fromVersion="1.0" save={save} />);
    await userEvent.clear(screen.getByLabelText(/^Title/));
    await userEvent.click(screen.getByRole("button", { name: catalogText.editor.submit }));
    expect(save).not.toHaveBeenCalled();
    expect(screen.getByRole("alert")).toHaveTextContent(catalogText.editor.fixFirst(2));
    expect(screen.getByLabelText(/^New version/)).toHaveAttribute("aria-invalid", "true");
    expect(screen.getByLabelText(/^Title/)).toHaveAttribute("aria-invalid", "true");
  });

  it("editor sends the draft and shows the server's field and body reasons", async () => {
    const save = vi.fn().mockResolvedValue({
      ok: false, slug: "validation-failed", status: 400,
      params: [{ name: "version", reason: "must be 1 to 64 characters" }, { name: "body", reason: "clips must not be empty" }],
    });
    render(<CatalogEditor orgId="o" initial={small()} fromVersion={null} save={save} />);
    await userEvent.type(screen.getByLabelText(/^New version/), "2026.11.1");
    await userEvent.click(screen.getByRole("button", { name: catalogText.editor.submit }));
    await waitFor(() => expect(save).toHaveBeenCalledTimes(1));
    expect(save.mock.calls[0]![0]).toEqual({ version: "2026.11.1", body: small() });
    const alert = await screen.findByRole("alert");
    expect(alert).toHaveTextContent("clips must not be empty");
    expect(screen.getByText("must be 1 to 64 characters", { selector: "span" })).toBeInTheDocument();
    expect(push).not.toHaveBeenCalled();
  });

  it("editor navigates to the new version on success and adds an AOI with roles", async () => {
    const save = vi.fn().mockResolvedValue({ ok: true, version: "v2" });
    render(<CatalogEditor orgId="o" initial={small()} fromVersion={null} save={save} />);
    await userEvent.click(screen.getByRole("button", { name: catalogText.aoi.add }));
    await userEvent.click(screen.getByRole("checkbox", { name: "Eye" }));
    await userEvent.type(screen.getByLabelText(/^New version/), "v2");
    await userEvent.click(screen.getByRole("button", { name: catalogText.editor.submit }));
    await waitFor(() => expect(push).toHaveBeenCalledWith("/o/config/catalog/v2?created=1"));
    const sent = save.mock.calls[0]![0].body as StimulusCatalog;
    expect(sent.clips[0]!.areas[1]).toMatchObject({ name: "Area_2", roles: ["eye"], endSec: 8 });
  });

  it("battery panel reorders and removes clips", async () => {
    const onChange = vi.fn();
    render(<BatteryPanel catalog={small()} onChange={onChange} />);
    const express = screen.getByRole("region", { name: "express_30s" });
    expect(within(express).getByText(catalogText.batteries.summary(2, 14))).toBeInTheDocument();
    await userEvent.click(within(express).getByRole("button", { name: `${catalogText.batteries.down}: b` }));
    expect(batteriesOf(onChange.mock.calls[0]![0]).find((b) => b.id === "express_30s")!.clipIds).toEqual(["a", "b"]);
    await userEvent.click(within(express).getByRole("button", { name: `${catalogText.batteries.remove}: a` }));
    expect(onChange.mock.calls[1]![0].clips[0].protocols).toEqual([]);
  });

  it("read-only battery panel has no controls", () => {
    render(<BatteryPanel catalog={small()} />);
    expect(screen.queryByRole("button")).toBeNull();
  });

  it("lifecycle offers the steps of the state and shows a four-eyes refusal", async () => {
    expect(stepsFor("draft")).toEqual(["approve", "retire"]);
    expect(stepsFor("candidate")).toEqual(["publish", "retire"]);
    expect(stepsFor("retired")).toEqual([]);
    const run = vi.fn().mockResolvedValue({ ok: false, slug: "four-eyes-required", status: 403, detail: "the author of a document cannot approve it" });
    render(<LifecycleActions status="draft" run={run} />);
    expect(screen.queryByRole("button", { name: "Publish" })).toBeNull();
    await userEvent.click(screen.getByRole("button", { name: "Approve" }));
    expect(run).toHaveBeenCalledWith("approve");
    expect(await screen.findByText(/Four-eyes rule/)).toHaveTextContent("cannot approve it");
  });

  it("retire asks for confirmation", async () => {
    const run = vi.fn().mockResolvedValue({ ok: true, status: "retired" });
    render(<LifecycleActions status="published" run={run} />);
    await userEvent.click(screen.getByRole("button", { name: "Retire" }));
    expect(run).not.toHaveBeenCalled();
    await userEvent.click(screen.getByRole("button", { name: "Yes, retire" }));
    expect(run).toHaveBeenCalledWith("retire");
    expect(await screen.findByText(catalogText.lifecycle.done.retire)).toBeInTheDocument();
  });

  it("diff view lists the changes", () => {
    render(<CatalogDiffView from="1" to="2" diff={{ fields: [], added: ["c"], removed: [], changed: [{ id: "a", fields: ["areas"] }] }} />);
    expect(screen.getByText("a: areas")).toBeInTheDocument();
    expect(screen.getByText("c")).toBeInTheDocument();
  });
});

describe("RBAC", () => {
  it("catalog is read by owner, admin, clinician, researcher and superadmin; changed by superadmin only", () => {
    const readers = ROLES.filter((r) => can(r, "config.read"));
    expect(readers).toEqual(["owner", "admin", "clinician", "researcher", "superadmin"]);
    expect(ROLES.filter((r) => can(r, "config.manage"))).toEqual(["superadmin"]);
    expect(navItems("/o", "researcher").some((l) => l.href === "/o/config/catalog")).toBe(true);
    expect(navItems("/o", "viewer").some((l) => l.href === "/o/config/catalog")).toBe(false);
  });
});
