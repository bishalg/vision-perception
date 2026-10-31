/**
 * WP-C.13 contract and client tests: the clinical zod schemas against docs/spec/schemas/asd/api-v1.json, the live
 * client's wire format (If-Match, paths, problem slugs), and the mock client's sign-off rules.
 */
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it, vi } from "vitest";
import type { ZodType } from "zod";
import { ApiError, PROBLEM_SLUGS } from "@/lib/api";
import { createLiveClient } from "@/lib/api/live";
import { createMockClient, MOCK_ORG_ID } from "@/lib/api/mock";
import {
  COSIGN_POLICIES,
  CLINICAL_STATES,
  DISPOSITIONS,
  PAPERS,
  PATIENT_REF_MODES,
  REVIEW_VERSION_STATUSES,
  SIGNATURE_MEANING,
  SIGNATURE_ROLES,
  WORKLIST_ASSIGNED,
  WORKLIST_SCOPES,
  amendClinicalReviewRequestSchema,
  claimClinicalReviewRequestSchema,
  clinicalReviewSchema,
  clinicalReviewVersionSchema,
  clinicalSignatureSchema,
  signRequestSchema,
  updateClinicalReviewRequestSchema,
  updateProfileRequestSchema,
  userProfileSchema,
  worklistItemSchema,
  worklistPageSchema,
} from "@/lib/api/clinical";

interface Def {
  enum?: string[];
  examples?: unknown[];
  required?: string[];
  properties?: Record<string, unknown>;
}
const defs = (
  JSON.parse(readFileSync(resolve(process.cwd(), "../../docs/spec/schemas/asd/api-v1.json"), "utf8")) as {
    $defs: Record<string, Def>;
  }
).$defs;

type Shaped = ZodType & { shape: Record<string, ZodType & { isOptional(): boolean }> };
const OBJECTS: Record<string, Shaped> = {
  WorklistItem: worklistItemSchema,
  WorklistPage: worklistPageSchema,
  ClinicalReview: clinicalReviewSchema,
  ClinicalReviewVersion: clinicalReviewVersionSchema,
  ClinicalSignature: clinicalSignatureSchema,
  ClaimClinicalReviewRequest: claimClinicalReviewRequestSchema,
  UpdateClinicalReviewRequest: updateClinicalReviewRequestSchema,
  SignRequest: signRequestSchema,
  AmendClinicalReviewRequest: amendClinicalReviewRequestSchema,
  UserProfile: userProfileSchema,
  UpdateProfileRequest: updateProfileRequestSchema,
} as never;

describe("clinical DTOs match api-v1.json", () => {
  for (const [name, schema] of Object.entries(OBJECTS)) {
    it(`${name}: zod accepts published examples and declares every property`, () => {
      const def = defs[name];
      expect(def, `${name} missing from api-v1.json`).toBeDefined();
      for (const example of def!.examples ?? []) expect(schema.parse(example)).toBeTruthy();
      for (const p of Object.keys(def!.properties ?? {})) {
        expect(schema.shape, `${name}.${p} not declared in zod`).toHaveProperty(p);
      }
      for (const p of def!.required ?? []) {
        expect(schema.shape[p]!.isOptional(), `${name}.${p} is required by the contract`).toBe(false);
      }
    });
  }

  it("enums match the contract", () => {
    expect([...DISPOSITIONS]).toEqual(defs.Disposition!.enum);
    expect([...CLINICAL_STATES]).toEqual(defs.ClinicalReviewState!.enum);
    expect([...REVIEW_VERSION_STATUSES]).toEqual(defs.ReviewVersionStatus!.enum);
    expect([...SIGNATURE_ROLES]).toEqual(defs.SignatureRole!.enum);
    expect([...COSIGN_POLICIES]).toEqual(defs.CosignPolicy!.enum);
    expect([...WORKLIST_SCOPES]).toEqual(defs.WorklistScope!.enum);
    expect([...WORKLIST_ASSIGNED]).toEqual(defs.WorklistAssigned!.enum);
    expect([...PATIENT_REF_MODES]).toEqual(defs.PatientRefMode!.enum);
    expect([...PAPERS]).toEqual(defs.Paper!.enum);
  });

  it("every clinical problem slug the UI messages handle is in the contract", () => {
    for (const slug of ["content-changed", "stale-version", "reauthentication-failed", "cosigner-must-differ", "signer-profile-incomplete", "clinical-crypto-unavailable"]) {
      expect(PROBLEM_SLUGS).toContain(slug);
    }
  });

  it("the meaning statement is the one SIGN-5 specifies", () => {
    expect(SIGNATURE_MEANING).toBe("I have reviewed this observation record and approve this review.");
  });

  it("a review without ClinicalContentRead (no impression/recommendations) is accepted", () => {
    const base = {
      id: "r", org_id: "o", session_id: "s", state: "in_progress", cosign_required: false, row_version: 2,
      current: { version_no: 1, status: "draft", created_at: "2026-11-01T00:00:00Z" },
      content_hash: "ab".repeat(32), signatures: [], created_at: "2026-11-01T00:00:00Z", updated_at: "2026-11-01T00:00:00Z",
    };
    expect(clinicalReviewSchema.parse(base).current.impression).toBeUndefined();
    expect(clinicalReviewSchema.safeParse({ ...base, state: "done" }).success).toBe(false);
  });
});

describe("live clinical client", () => {
  function client(responder: (url: string, init: RequestInit) => Response) {
    const calls: { url: string; init: RequestInit }[] = [];
    const fetchImpl = vi.fn(async (url: string, init: RequestInit) => {
      calls.push({ url, init });
      return responder(url, init);
    }) as unknown as typeof fetch;
    return { api: createLiveClient({ baseUrl: "https://api.test", accessToken: "tok", fetchImpl }), calls };
  }
  const review = {
    id: "r1", org_id: "o", session_id: "s1", state: "in_progress", cosign_required: false, row_version: 3,
    current: { version_no: 1, status: "draft", created_at: "2026-11-01T00:00:00Z", disposition: "monitor" },
    content_hash: "cd".repeat(32), signatures: [], created_at: "2026-11-01T00:00:00Z", updated_at: "2026-11-01T00:00:00Z",
  };
  const json = (body: unknown, status = 200) =>
    new Response(JSON.stringify(body), { status, headers: { "Content-Type": "application/json" } });
  const problem = (status: number, slug: string) =>
    json({ type: `https://schemas.curv.org/asd/problems/${slug}`, title: slug, status }, status);

  it("saves a draft with If-Match: \"<row_version>\" and the PATCH body", async () => {
    const { api, calls } = client(() => json(review));
    await api.saveClinicalDraft("r1", 3, { disposition: "monitor", impression: "x" });
    expect(calls[0]!.url).toBe("https://api.test/v1/clinical-reviews/r1");
    expect(calls[0]!.init.method).toBe("PATCH");
    expect((calls[0]!.init.headers as Record<string, string>)["If-Match"]).toBe('"3"');
    expect(JSON.parse(calls[0]!.init.body as string)).toEqual({ disposition: "monitor", impression: "x" });
  });

  it("posts sign and cosign to their endpoints and amend with the reason", async () => {
    const { api, calls } = client(() => json(review));
    const body = { content_hash: "cd".repeat(32), password: "pw", meaning_ack: true };
    await api.signClinicalReview("r1", body);
    await api.cosignClinicalReview("r1", body);
    await api.amendClinicalReview("r1", { reason: "Corrected the disposition" });
    expect(calls.map((c) => c.url)).toEqual([
      "https://api.test/v1/clinical-reviews/r1/sign",
      "https://api.test/v1/clinical-reviews/r1/cosign",
      "https://api.test/v1/clinical-reviews/r1/amend",
    ]);
    expect(JSON.parse(calls[0]!.init.body as string)).toEqual(body);
  });

  it("answers null for a session nobody has claimed (404) but rethrows other errors", async () => {
    const missing = client(() => problem(404, "not-found"));
    expect(await missing.api.getSessionClinicalReview("s1")).toBeNull();
    const forbidden = client(() => problem(403, "forbidden"));
    await expect(forbidden.api.getSessionClinicalReview("s1")).rejects.toMatchObject({ status: 403 });
  });

  it("surfaces problem slugs from sign failures", async () => {
    for (const [status, slug] of [[409, "content-changed"], [412, "stale-version"], [401, "reauthentication-failed"], [409, "cosigner-must-differ"], [409, "signer-profile-incomplete"], [503, "clinical-crypto-unavailable"]] as const) {
      const { api } = client(() => problem(status, slug));
      const err = await api.signClinicalReview("r1", { content_hash: "h", password: "p", meaning_ack: true }).catch((e) => e);
      expect(err).toBeInstanceOf(ApiError);
      expect(err.slug).toBe(slug);
      expect(err.status).toBe(status);
    }
  });

  it("builds the worklist query and parses the page", async () => {
    const { api, calls } = client(() => json({ items: [], next_cursor: null, total: 0 }));
    await api.getWorklist("o1", { state: "open", assigned: "me", limit: 50 });
    expect(calls[0]!.url).toBe("https://api.test/v1/orgs/o1/worklist?state=open&assigned=me&limit=50");
  });

  it("patches org settings as a merge patch under `settings`", async () => {
    const { api, calls } = client(() => json({ id: "o1", name: "N", kind: "clinic" }));
    await api.updateOrgClinicalSettings("o1", { clinical: { cosign_policy: "always" }, paper: "letter" });
    expect(JSON.parse(calls[0]!.init.body as string)).toEqual({ settings: { clinical: { cosign_policy: "always" }, paper: "letter" } });
  });
});

describe("mock clinical client", () => {
  const SIGN = { password: "demo", meaning_ack: true };

  it("serves a contract-valid worklist that excludes final reviews", async () => {
    const api = createMockClient();
    const page = await api.getWorklist(MOCK_ORG_ID, { limit: 100 });
    worklistPageSchema.parse(page);
    expect(page.items.some((w) => w.clinical_state === "awaiting_cosign")).toBe(true);
    expect(page.items.every((w) => w.clinical_state !== "final")).toBe(true);
    const mine = await api.getWorklist(MOCK_ORG_ID, { assigned: "me" });
    expect(mine.items.every((w) => w.assigned_to !== null)).toBe(true);
  });

  it("claim -> save (If-Match) -> sign reaches final and shows a signature", async () => {
    const api = createMockClient();
    const sessionId = (await api.getWorklist(MOCK_ORG_ID)).items.find((w) => !w.clinical_state)!.id;
    expect(await api.getSessionClinicalReview(sessionId)).toBeNull();
    const claimed = clinicalReviewSchema.parse(await api.claimClinicalReview(sessionId));
    expect(claimed.state).toBe("open");
    await expect(api.saveClinicalDraft(claimed.id, claimed.row_version + 5, { disposition: "monitor" })).rejects.toMatchObject({ status: 412 });
    const saved = await api.saveClinicalDraft(claimed.id, claimed.row_version, { disposition: "monitor", impression: "Synthetic." });
    expect(saved.state).toBe("in_progress");
    expect(saved.row_version).toBe(claimed.row_version + 1);
    expect(saved.content_hash).not.toBe(claimed.content_hash);
    const signed = await api.signClinicalReview(saved.id, { ...SIGN, content_hash: saved.content_hash });
    expect(signed.state).toBe("final");
    expect(signed.signatures).toHaveLength(1);
    expect(signed.signatures[0]!.meaning).toBe(SIGNATURE_MEANING);
    expect((await api.getWorklist(MOCK_ORG_ID, { limit: 100 })).items.some((w) => w.id === sessionId)).toBe(false);
  });

  it("refuses a wrong password, a stale hash and a missing display name with the contract slugs", async () => {
    const api = createMockClient();
    const sessionId = (await api.getWorklist(MOCK_ORG_ID)).items.find((w) => !w.clinical_state)!.id;
    const r = await api.saveClinicalDraft((await api.claimClinicalReview(sessionId)).id, 1, { disposition: "monitor" });
    const slug = async (body: Parameters<typeof api.signClinicalReview>[1]) =>
      (await api.signClinicalReview(r.id, body).catch((e: ApiError) => e)) as ApiError;
    expect((await slug({ ...SIGN, password: "nope", content_hash: r.content_hash })).slug).toBe("reauthentication-failed");
    expect((await slug({ ...SIGN, content_hash: "0".repeat(64) })).slug).toBe("content-changed");
    await api.updateMyProfile({ display_name: "" });
    expect((await slug({ ...SIGN, content_hash: r.content_hash })).slug).toBe("signer-profile-incomplete");
  });

  it("cosign by the same user is refused; the seeded awaiting_cosign review accepts the demo user", async () => {
    const api = createMockClient();
    const waiting = (await api.getWorklist(MOCK_ORG_ID, { state: "awaiting_cosign" })).items[0]!;
    const r = await api.getClinicalReview(waiting.review_id!);
    const done = await api.cosignClinicalReview(r.id, { ...SIGN, content_hash: r.content_hash });
    expect(done.state).toBe("final");
    // Same-user case: sign with cosign policy always, then cosign as the same user.
    await api.updateOrgClinicalSettings(MOCK_ORG_ID, { clinical: { cosign_policy: "always" } });
    const sessionId = (await api.getWorklist(MOCK_ORG_ID)).items.find((w) => !w.clinical_state)!.id;
    const c = await api.claimClinicalReview(sessionId);
    expect(c.cosign_required).toBe(true);
    const s = await api.saveClinicalDraft(c.id, 1, { disposition: "monitor" });
    const first = await api.signClinicalReview(s.id, { ...SIGN, content_hash: s.content_hash });
    expect(first.state).toBe("awaiting_cosign");
    const again = (await api.cosignClinicalReview(s.id, { ...SIGN, content_hash: first.content_hash }).catch((e: ApiError) => e)) as ApiError;
    expect(again.slug).toBe("cosigner-must-differ");
  });

  it("amend needs a 10..500 character reason and starts version 2", async () => {
    const api = createMockClient();
    const final = (await api.getSessionClinicalReview((await listAll(api)).find((s) => s.final)!.id))!;
    expect(final.state).toBe("final");
    await expect(api.amendClinicalReview(final.id, { reason: "short" })).rejects.toMatchObject({ status: 400 });
    const amended = await api.amendClinicalReview(final.id, { reason: "Corrected the recommendation text." });
    expect(amended.state).toBe("amending");
    expect(amended.current.version_no).toBe(final.current.version_no + 1);
    expect(amended.current.amend_reason).toBe("Corrected the recommendation text.");
  });
});

async function listAll(api: ReturnType<typeof createMockClient>): Promise<{ id: string; final: boolean }[]> {
  const out = [];
  for (const s of (await api.listSessions(MOCK_ORG_ID, { limit: 10 })).items) {
    const r = await api.getSessionClinicalReview(s.id);
    out.push({ id: s.id, final: r?.state === "final" });
  }
  return out;
}
