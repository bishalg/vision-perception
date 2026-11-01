/**
 * Fixture-backed config documents for mock mode (WP-C.4). Seeds a realistic stimulus catalog history (a retired
 * bundled version, the published one, a candidate and a draft) and mimics the lifecycle answers the UI surfaces:
 * version format, shape errors as `body` reasons, `409 config-version-exists`, `409 invalid-transition` and the
 * four-eyes rule (`403 four-eyes-required`). The real rules live in the Rust backend (`config_docs::lifecycle`).
 *
 * The mock has one signed-in user id for every login, so a draft created here cannot be approved by "you" (four-eyes)
 * while the seeded draft and candidate, authored by someone else, can.
 */
import { ApiError, PROBLEM_BASE } from "../types";
import {
  stimulusCatalogSchema,
  type CatalogApi,
  type ConfigDocument,
  type ConfigDocumentKind,
  type StimulusCatalog,
} from "../catalog";
import { SEED_CATALOG } from "./catalogSeed";

export interface MockCatalogContext {
  userId: string;
  now: number;
  page: <T>(
    rows: T[],
    key: (row: T) => [string, string],
    cursor: string | undefined,
    limit?: number,
  ) => { slice: T[]; next_cursor: string | null; total: number };
}

/** Synthetic colleagues: the author and approver of the seeded documents. */
export const MOCK_CONFIG_AUTHOR_ID = "a0c0ffee-0000-4000-8000-00000000c0a1";
export const MOCK_CONFIG_APPROVER_ID = "a0c0ffee-0000-4000-8000-00000000c0a2";

/** Seeded catalog versions, oldest first. */
export const MOCK_CATALOG_VERSIONS = {
  bundled: "bundled-1.0.0",
  published: "2026.10.1",
  candidate: "2026.11.0-rc1",
  draft: "2026.11.0-rc2",
} as const;

function problem(status: number, slug: string, title: string, detail?: string, params?: { name: string; reason: string }[]) {
  return new ApiError(status, {
    type: `${PROBLEM_BASE}${slug}`,
    title,
    status,
    detail,
    ...(params ? { invalid_params: params } : {}),
  });
}

const VERSION_RE = /^[A-Za-z0-9._-]{1,64}$/;
const clone = <T,>(v: T): T => JSON.parse(JSON.stringify(v)) as T;

/** Real measured durations by the id's `_<n>s` suffix (the bundled file says 8.0 s for every clip). */
function measured(c: StimulusCatalog): StimulusCatalog {
  const next = clone(c);
  for (const clip of next.clips) {
    const m = /_(\d+)s$/.exec(clip.id);
    if (m) clip.durationSec = Number(m[1]);
    for (const a of clip.areas) a.endSec = Math.min(a.endSec, clip.durationSec);
  }
  return next;
}

function seedCatalogs(): { published: StimulusCatalog; candidate: StimulusCatalog; draft: StimulusCatalog } {
  const published = measured(SEED_CATALOG);
  const candidate = clone(published);
  // rc1: the gears clip joins the comprehensive battery and the turn-taking clip gets an eye AOI.
  candidate.clips.find((c) => c.id === "stim_06_dynamic_gears_marbles_8s")!.protocols.push({ id: "comprehensive_60s", order: 8 });
  candidate.clips
    .find((c) => c.id === "stim_09_social_turn_taking_10s")!
    .areas.push({ name: "Speaker_Eyes", startSec: 0, endSec: 10, xMin: 0.38, yMin: 0.22, xMax: 0.62, yMax: 0.36, roles: ["social", "eye"] });
  const draft = clone(candidate);
  // rc2: adds a name-call orienting clip at the end of the express battery.
  draft.clips.push({
    id: "stim_11_name_call_orient_6s",
    title: "Name Call Orienting",
    category: "Orienting",
    durationSec: 6,
    minMonths: 12,
    maxMonths: 60,
    protocols: [{ id: "express_30s", order: 4 }],
    areas: [
      { name: "Caller_Face", startSec: 1.5, endSec: 6, xMin: 0.6, yMin: 0.15, xMax: 0.92, yMax: 0.6, roles: ["social", "orienting"] },
      { name: "Toy_Distractor", startSec: 0, endSec: 6, xMin: 0.08, yMin: 0.45, xMax: 0.35, yMax: 0.85, roles: ["geometric"] },
    ],
  });
  return { published, candidate, draft };
}

interface Row {
  doc: ConfigDocument;
  approvedBy: string | null;
}

export function createMockCatalog(ctx: MockCatalogContext): CatalogApi {
  const { userId, now } = ctx;
  const iso = (daysAgo: number) => new Date(now - daysAgo * 86_400_000).toISOString();
  const { published, candidate, draft } = seedCatalogs();
  const v = MOCK_CATALOG_VERSIONS;
  const doc = (n: number, over: Partial<ConfigDocument> & Pick<ConfigDocument, "version" | "status" | "body">): ConfigDocument => ({
    id: `c0a7a10c-0000-4000-8000-${String(n).padStart(12, "0")}`,
    kind: "stimulus_catalog",
    created_by: MOCK_CONFIG_AUTHOR_ID,
    created_at: iso(10),
    published_at: null,
    notes: null,
    ...over,
  });
  // Newest first, as the backend lists them.
  const rows: Row[] = [
    {
      doc: doc(4, { version: v.draft, status: "draft", body: draft, created_at: iso(1), notes: "Adds the name-call orienting clip to Express." }),
      approvedBy: null,
    },
    {
      doc: doc(3, { version: v.candidate, status: "candidate", body: candidate, created_at: iso(3), notes: "Gears clip in Comprehensive; speaker-eyes AOI." }),
      approvedBy: MOCK_CONFIG_APPROVER_ID,
    },
    {
      doc: doc(2, { version: v.published, status: "published", body: published, created_at: iso(24), published_at: iso(22), notes: "Measured clip durations." }),
      approvedBy: MOCK_CONFIG_APPROVER_ID,
    },
    {
      doc: doc(1, { version: v.bundled, status: "retired", body: clone(SEED_CATALOG), created_by: null, created_at: iso(60), published_at: iso(59), notes: "Seeded from the bundled kit." }),
      approvedBy: MOCK_CONFIG_APPROVER_ID,
    },
  ];
  let seq = rows.length;

  const find = (kind: ConfigDocumentKind, version: string) => {
    const row = rows.find((r) => r.doc.kind === kind && r.doc.version === version);
    if (!row) throw problem(404, "not-found", "Not found", "config document not found");
    return row;
  };
  const wrongState = (row: Row, step: string, expected: string) =>
    problem(409, "invalid-transition", "Conflict", `cannot ${step} a ${row.doc.status} document (expected ${expected})`);
  const fourEyes = (detail: string) => problem(403, "four-eyes-required", "Forbidden", detail);

  return {
    async listConfigDocuments(kind, q) {
      const all = rows.filter((r) => r.doc.kind === kind && (!q?.status || r.doc.status === q.status)).map((r) => r.doc);
      const { slice, next_cursor } = ctx.page(all, (d) => [d.created_at, d.id], q?.cursor, q?.limit);
      return { items: clone(slice), next_cursor, total: null };
    },
    async getConfigDocument(kind, version) {
      return clone(find(kind, version).doc);
    },
    async createConfigDocument(kind, req) {
      if (!VERSION_RE.test(req.version)) {
        throw problem(400, "validation-failed", "Validation failed", undefined, [
          { name: "version", reason: "must be 1 to 64 characters of letters, digits, '.', '_' or '-'" },
        ]);
      }
      if (kind === "stimulus_catalog") {
        const parsed = stimulusCatalogSchema.safeParse(req.body);
        if (!parsed.success) {
          const i = parsed.error.issues[0]!;
          throw problem(400, "validation-failed", "Validation failed", undefined, [
            { name: "body", reason: `${i.message} at ${i.path.join(".")}` },
          ]);
        }
        if (parsed.data.clips.length === 0) {
          throw problem(400, "validation-failed", "Validation failed", undefined, [{ name: "body", reason: "clips must not be empty" }]);
        }
      }
      if (rows.some((r) => r.doc.kind === kind && r.doc.version === req.version)) {
        throw problem(409, "config-version-exists", "Conflict", `${kind} version \`${req.version}\` already exists`);
      }
      seq += 1;
      const created: ConfigDocument = {
        id: `c0a7a10c-0000-4000-8000-${String(seq).padStart(12, "0")}`,
        kind,
        version: req.version,
        status: "draft",
        body: clone(req.body),
        created_by: userId,
        created_at: new Date(Math.max(Date.now(), now)).toISOString(),
        published_at: null,
        notes: req.notes ?? null,
      };
      rows.unshift({ doc: created, approvedBy: null });
      return clone(created);
    },
    async transitionConfigDocument(kind, version, step) {
      const row = find(kind, version);
      const author = row.doc.created_by ?? null;
      if (step === "approve") {
        if (row.doc.status !== "draft") throw wrongState(row, "approve", "draft");
        if (author === userId) throw fourEyes("the author of a document cannot approve it");
        row.doc.status = "candidate";
        row.approvedBy = userId;
      } else if (step === "publish") {
        if (row.doc.status !== "candidate") throw wrongState(row, "publish", "candidate");
        if (author === userId) throw fourEyes("the author of a document cannot publish it");
        if (author === null && row.approvedBy === userId) {
          throw fourEyes("a system-seeded document is published by someone other than its approver");
        }
        for (const r of rows) if (r.doc.kind === kind && r.doc.status === "published") r.doc.status = "retired";
        row.doc.status = "published";
        row.doc.published_at = new Date(Math.max(Date.now(), now)).toISOString();
      } else {
        if (row.doc.status === "retired") throw wrongState(row, "retire", "draft, candidate or published");
        row.doc.status = "retired";
      }
      return clone(row.doc);
    },
  };
}
