/**
 * Drift guard for the EMR-lite record schemas (WP-C.18, CTR-3), next to `participant.contract.test.ts`: the zod schemas
 * in `emr.ts` (and the TRS-1 request in `admin.ts`) must declare every property of docs/spec/schemas/asd/api-v1.json,
 * require the required ones, accept every published example and agree with its enums; the mock's answers must parse.
 */
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";
import type { ZodType } from "@/lib/zod";
import { totpResetRequestSchema } from "./admin";
import { createMockClient, MOCK_ORG_ID } from "./mock";
import {
  ATTACHMENT_KINDS,
  ATTACHMENT_MEDIA_TYPES,
  ENCOUNTER_KINDS,
  ENCOUNTER_STATUSES,
  attachmentSchema,
  attachmentUploadSchema,
  createAttachmentRequestSchema,
  createEncounterRequestSchema,
  createParticipantRequestSchema,
  createdParticipantSchema,
  encounterPageSchema,
  encounterSchema,
  followUpPageSchema,
  followUpSchema,
  linkSessionRequestSchema,
  participantSearchPageSchema,
  participantSearchRequestSchema,
  participantSearchRowSchema,
  updateEncounterRequestSchema,
  uploadTargetSchema,
} from "./emr";

interface Def {
  enum?: string[];
  examples?: unknown[];
  required?: string[];
  properties?: Record<string, unknown>;
}
const api = JSON.parse(
  readFileSync(resolve(process.cwd(), "../../docs/spec/schemas/asd/api-v1.json"), "utf8"),
) as { $defs: Record<string, Def> };
const defs = api.$defs;

type Shaped = ZodType & { shape: Record<string, ZodType & { isOptional(): boolean }> };

const SCHEMAS: Record<string, Shaped> = {
  Encounter: encounterSchema,
  EncounterPage: encounterPageSchema,
  CreateEncounterRequest: createEncounterRequestSchema,
  UpdateEncounterRequest: updateEncounterRequestSchema,
  LinkSessionRequest: linkSessionRequestSchema,
  FollowUp: followUpSchema,
  FollowUpPage: followUpPageSchema,
  Attachment: attachmentSchema,
  CreateAttachmentRequest: createAttachmentRequestSchema,
  UploadTarget: uploadTargetSchema,
  AttachmentUpload: attachmentUploadSchema,
  ParticipantSearchRequest: participantSearchRequestSchema,
  ParticipantSearchRow: participantSearchRowSchema,
  ParticipantSearchPage: participantSearchPageSchema,
  CreateParticipantRequest: createParticipantRequestSchema,
  CreatedParticipant: createdParticipantSchema,
  TotpResetRequest: totpResetRequestSchema,
} as never;

describe("api-v1.json contract: EMR-lite record (WP-C.18)", () => {
  for (const [name, schema] of Object.entries(SCHEMAS)) {
    const def = defs[name];
    it(`${name}: zod declares every property, requires the required ones and nothing more`, () => {
      expect(def, `${name} missing from api-v1.json`).toBeDefined();
      const props = Object.keys(def!.properties ?? {});
      for (const p of props) expect(schema.shape, `${name}.${p} not declared in zod`).toHaveProperty(p);
      for (const p of Object.keys(schema.shape)) expect(props, `${name}.${p} is not in the contract`).toContain(p);
      for (const p of def!.required ?? []) {
        expect(schema.shape[p]!.isOptional(), `${name}.${p} is required by the contract`).toBe(false);
      }
      for (const p of Object.keys(schema.shape)) {
        if (!(def!.required ?? []).includes(p)) {
          expect(schema.shape[p]!.isOptional(), `${name}.${p} is optional in the contract`).toBe(true);
        }
      }
      for (const example of def!.examples ?? []) expect(schema.parse(example)).toBeTruthy();
    });
  }

  it("enums match the contract", () => {
    expect([...ENCOUNTER_KINDS]).toEqual(defs.EncounterKind!.enum);
    expect([...ENCOUNTER_STATUSES]).toEqual(defs.EncounterStatus!.enum);
    expect([...ATTACHMENT_KINDS]).toEqual(defs.AttachmentKind!.enum);
    expect([...ATTACHMENT_MEDIA_TYPES]).toEqual(defs.AttachmentMediaType!.enum);
  });

  it("every problem slug the record UI maps is a contract slug", () => {
    const slugs = (defs.ProblemSlug?.enum ?? []) as string[];
    for (const s of [
      "encounter-closed", "encounter-summary-forbidden", "attachment-too-large", "attachment-type-not-allowed",
      "digest-mismatch", "stale-version", "totp-reset-forbidden", "reauthentication-failed",
    ]) {
      expect(slugs, s).toContain(s);
    }
  });

  it("the mock returns shapes the schemas accept", async () => {
    const mock = createMockClient();
    const created = createdParticipantSchema.parse(await mock.createParticipant(MOCK_ORG_ID, { language: "ne" }));
    expect(created.qr_payload).toBe(`asd://participant/${created.participant_token}?org=${MOCK_ORG_ID}`);
    const search = participantSearchPageSchema.parse(await mock.searchParticipants(MOCK_ORG_ID, { limit: 5 }));
    expect(search.items.length).toBeGreaterThan(0);
    const first = (await mock.listParticipants(MOCK_ORG_ID, { limit: 100 })).items.find((p) => p.session_count > 0)!;
    const encounters = encounterPageSchema.parse(await mock.listEncounters(first.id));
    expect(encounters.items.length).toBeGreaterThan(0);
    encounterSchema.parse(await mock.getEncounter(encounters.items[0]!.id));
    encounterSchema.parse(await mock.createEncounter(first.id, { kind: "phone_call", occurred_at: "2026-11-01T09:00:00Z" }));
    followUpPageSchema.parse(await mock.listFollowUps(MOCK_ORG_ID));
    attachmentUploadSchema.parse(
      await mock.createAttachment(first.id, { kind: "referral", file_name: "a.pdf", media_type: "application/pdf", byte_size: 10 }),
    );
  });
});
